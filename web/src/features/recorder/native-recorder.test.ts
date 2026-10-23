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
		pause: vi.fn(async () => ({
			state: "recording" as const,
			paused: true,
			elapsedS: 40,
			uploadsWaiting: 0,
			uploadsFailed: 0,
			notice: "",
		})),
		resume: vi.fn(async () => ({
			state: "recording" as const,
			paused: false,
			elapsedS: 40,
			uploadsWaiting: 0,
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
		takePendingBookmarks: vi.fn(async () => ({ bookmarks: [] })),
		ackBookmarks: vi.fn(async () => undefined),
		getTranscript: vi.fn(async () => ({
			sessionId: null,
			entries: [],
			status: {},
		})),
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

	it("asks Swift for the session position when bookmarking, not the page's clock", async () => {
		const { plugin, emit } = fakePlugin();
		const recorder = new NativeRecorder(plugin);
		await recorder.attach();
		await recorder.start("key1", 1);
		expect(recorder.getState().sessionId).toBe("s1");
		emit("state", { state: "recording" });
		emit("elapsed", { elapsedS: 30 });

		// Locked for five minutes: Swift sent no `elapsed`, and the segments
		// closed meanwhile replay on unlock while the page still shows 30 s.
		for (let index = 0; index < 5; index += 1) {
			emit("segment", { index, durationS: 60 });
		}
		vi.mocked(plugin.getStatus).mockResolvedValueOnce({
			state: "recording",
			elapsedS: 330.6,
			positionS: 330.2,
			uploadsWaiting: 0,
			uploadsFailed: 0,
			notice: "",
		});
		expect(recorder.getState().elapsedS).toBe(30);
		expect(await recorder.position()).toBe(330.2);
	});

	it("pauses and resumes from the page and follows a pause from the lock screen", async () => {
		const { plugin, emit } = fakePlugin({ state: "recording", paused: false });
		const recorder = new NativeRecorder(plugin);
		await recorder.attach();
		expect(recorder.getState()).toMatchObject({
			canPause: true,
			paused: false,
		});

		await recorder.togglePause();
		expect(plugin.pause).toHaveBeenCalledTimes(1);
		expect(recorder.getState().paused).toBe(true);
		await recorder.togglePause();
		expect(plugin.resume).toHaveBeenCalledTimes(1);
		expect(recorder.getState().paused).toBe(false);

		// Lock screen or AirPods: Swift says so while the page is on screen.
		emit("paused", { paused: true });
		expect(recorder.getState().paused).toBe(true);
		// The session ended: never shown as paused afterwards.
		emit("state", { state: "idle" });
		expect(recorder.getState()).toMatchObject({
			recording: false,
			paused: false,
		});
	});

	it("reads a pause made while the page was suspended", async () => {
		const { plugin } = fakePlugin({
			state: "recording",
			paused: true,
			elapsedS: 80,
		});
		const recorder = new NativeRecorder(plugin);
		await recorder.attach();
		expect(recorder.getState()).toMatchObject({
			recording: true,
			paused: true,
		});
	});

	it("offers no pause to app builds that cannot pause", async () => {
		const { plugin } = fakePlugin({ state: "recording" });
		const recorder = new NativeRecorder(plugin);
		await recorder.attach();
		expect(recorder.getState().canPause).toBe(false);
		await recorder.togglePause();
		expect(plugin.pause).not.toHaveBeenCalled();
	});

	it("shows why a pause failed and keeps recording", async () => {
		const { plugin } = fakePlugin({ state: "recording", paused: false });
		vi.mocked(plugin.pause).mockRejectedValueOnce(new Error("bridge"));
		const recorder = new NativeRecorder(plugin);
		await recorder.attach();
		await recorder.togglePause();
		expect(recorder.getState()).toMatchObject({
			recording: true,
			paused: false,
		});
		expect(recorder.getState().error).not.toBe("");
	});

	it("falls back to Swift's clock, then the page's, for older app builds", async () => {
		const { plugin, emit } = fakePlugin({ state: "recording", elapsedS: 95 });
		const recorder = new NativeRecorder(plugin);
		await recorder.attach();
		expect(await recorder.position()).toBe(95);

		emit("elapsed", { elapsedS: 97 });
		vi.mocked(plugin.getStatus).mockRejectedValueOnce(new Error("bridge"));
		expect(await recorder.position()).toBe(97);
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
		expect(await recorder.position()).toBe(12);
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
