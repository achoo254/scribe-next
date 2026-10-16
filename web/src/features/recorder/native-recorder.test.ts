import { describe, expect, it, vi } from "vitest";
import {
	NativeRecorder,
	type RecorderNativePlugin,
	userKeyOf,
} from "./native-recorder";

type Handler = (data: Record<string, unknown>) => void;

function fakePlugin(
	status: Partial<Awaited<ReturnType<RecorderNativePlugin["getStatus"]>>> = {},
) {
	const handlers = new Map<string, Set<Handler>>();
	const plugin: RecorderNativePlugin = {
		start: vi.fn(async () => ({ sessionId: "s1" })),
		stop: vi.fn(async () => ({
			state: "idle" as const,
			elapsedS: 61,
			uploadsWaiting: 2,
			uploadsFailed: 0,
			notice: "",
		})),
		getStatus: vi.fn(async () => ({
			state: "idle" as const,
			elapsedS: 0,
			uploadsWaiting: 0,
			uploadsFailed: 0,
			notice: "",
			...status,
		})),
		syncUploads: vi.fn(async () => ({ lostSegments: 0, lostSecondsMax: 0 })),
		signOut: vi.fn(async () => undefined),
		addListener: vi.fn(async (event: string, handler: Handler) => {
			const set = handlers.get(event) ?? new Set<Handler>();
			set.add(handler);
			handlers.set(event, set);
			return { remove: async () => void set.delete(handler) };
		}),
	};
	const emit = (event: string, data: Record<string, unknown> = {}) => {
		for (const handler of handlers.get(event) ?? []) handler(data);
	};
	const listenerCount = () =>
		[...handlers.values()].reduce((sum, set) => sum + set.size, 0);
	return { plugin, emit, listenerCount };
}

describe("NativeRecorder", () => {
	it("takes Swift's status as the truth when it attaches (WebView was suspended)", async () => {
		const { plugin } = fakePlugin({
			state: "recording",
			elapsedS: 125,
			uploadsWaiting: 1,
		});
		const recorder = new NativeRecorder(plugin);
		await recorder.attach();
		expect(recorder.getState()).toMatchObject({
			recording: true,
			elapsedS: 125,
			uploadsWaiting: 1,
		});
	});

	it("follows the plugin's events while recording", async () => {
		const { plugin, emit } = fakePlugin();
		const recorder = new NativeRecorder(plugin);
		await recorder.attach();
		await recorder.start("key1", 1);
		expect(plugin.start).toHaveBeenCalledWith({
			userKey: "key1",
			segmentMinutes: 1,
		});
		expect(recorder.getState().starting).toBe(true);

		emit("state", { state: "recording" });
		emit("elapsed", { elapsedS: 42 });
		emit("level", { level: 0.2 });
		emit("notice", { message: "Tạm dừng: cuộc gọi" });
		emit("uploads", { waiting: 3, failed: 1 });
		expect(recorder.getState()).toMatchObject({
			recording: true,
			starting: false,
			elapsedS: 42,
			notice: "Tạm dừng: cuộc gọi",
			uploadsWaiting: 3,
			uploadsFailed: 1,
		});
		expect(recorder.getLevel()).toBe(0.2);

		await recorder.stop();
		expect(recorder.getState()).toMatchObject({
			recording: false,
			uploadsWaiting: 2,
		});
		expect(recorder.getLevel()).toBe(0);
	});

	it("counts session seconds over closed segments for bookmarks", async () => {
		const { plugin, emit } = fakePlugin();
		const recorder = new NativeRecorder(plugin);
		await recorder.attach();
		await recorder.start("key1", 1);
		expect(recorder.getState().sessionId).toBe("s1");

		emit("state", { state: "recording" });
		emit("elapsed", { elapsedS: 30 });
		expect(recorder.position()).toBe(30);
		// The first segment closed at 60.4 s of clock, its file is 60 s long.
		emit("elapsed", { elapsedS: 60.4 });
		emit("segment", { index: 0, durationS: 60 });
		emit("elapsed", { elapsedS: 75.4 });
		expect(recorder.position()).toBeCloseTo(75);

		// A new session starts from zero.
		await recorder.stop();
		await recorder.start("key1", 1);
		emit("elapsed", { elapsedS: 3 });
		expect(recorder.position()).toBe(3);
	});

	it("reads the open session back from Swift's status", async () => {
		const { plugin } = fakePlugin({
			state: "recording",
			elapsedS: 12,
			sessionId: "s-live",
		});
		const recorder = new NativeRecorder(plugin);
		await recorder.attach();
		expect(recorder.getState().sessionId).toBe("s-live");
		expect(recorder.position()).toBe(12);
	});

	it("shows why a start failed and stops on an engine failure", async () => {
		const { plugin, emit } = fakePlugin();
		vi.mocked(plugin.start).mockRejectedValueOnce(
			new Error("Thiếu thông tin tài khoản."),
		);
		const recorder = new NativeRecorder(plugin);
		await recorder.attach();
		await recorder.start("", 1);
		expect(recorder.getState()).toMatchObject({
			starting: false,
			error: "Thiếu thông tin tài khoản.",
		});

		emit("state", { state: "recording" });
		emit("failure", { message: "Ứng dụng chưa được cấp quyền Micro" });
		expect(recorder.getState()).toMatchObject({
			recording: false,
			error: "Ứng dụng chưa được cấp quyền Micro",
		});
	});

	it("forwards uploads, expired sessions and a full quota", async () => {
		const { plugin, emit } = fakePlugin();
		const events = {
			onUploaded: vi.fn(),
			onNeedsSignIn: vi.fn(),
			onQuotaFull: vi.fn(),
		};
		const recorder = new NativeRecorder(plugin, events);
		await recorder.attach();
		emit("uploaded", { name: "s1#0" });
		emit("needsSignIn");
		emit("quotaFull", { message: "đã đầy quota 5GB" });
		expect(events.onUploaded).toHaveBeenCalledTimes(1);
		expect(events.onNeedsSignIn).toHaveBeenCalledTimes(1);
		expect(events.onQuotaFull).toHaveBeenCalledWith("đã đầy quota 5GB");
	});

	it("removes every listener on dispose", async () => {
		const { plugin, listenerCount } = fakePlugin();
		const recorder = new NativeRecorder(plugin);
		await recorder.attach();
		expect(listenerCount()).toBeGreaterThan(0);
		recorder.dispose();
		await Promise.resolve();
		expect(listenerCount()).toBe(0);
	});

	it("drops listeners of an attach that a dispose overtook (remount)", async () => {
		const { plugin, emit, listenerCount } = fakePlugin();
		const recorder = new NativeRecorder(plugin);
		const first = recorder.attach();
		recorder.dispose();
		await first;
		await Promise.resolve();
		expect(listenerCount()).toBe(0);

		await recorder.attach();
		const perMount = listenerCount();
		const seen = vi.fn();
		recorder.subscribe(seen);
		emit("elapsed", { elapsedS: 5 });
		expect(seen).toHaveBeenCalledTimes(1);
		expect(perMount).toBe(vi.mocked(plugin.addListener).mock.calls.length / 2);
	});

	it("shows the native start-up phase (permission prompt, engine warm-up)", async () => {
		const { plugin, emit } = fakePlugin({ state: "starting" });
		const recorder = new NativeRecorder(plugin);
		await recorder.attach();
		expect(recorder.getState()).toMatchObject({
			starting: true,
			recording: false,
		});
		emit("state", { state: "recording" });
		expect(recorder.getState()).toMatchObject({
			starting: false,
			recording: true,
		});
	});
});

describe("userKeyOf", () => {
	it("is stable per account and ignores case and spaces", async () => {
		const a = await userKeyOf("Dat@example.com");
		expect(a).toMatch(/^[0-9a-f]{24}$/);
		expect(await userKeyOf(" dat@example.com ")).toBe(a);
		expect(await userKeyOf("other@example.com")).not.toBe(a);
	});
});
