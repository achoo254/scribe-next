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
	const handlers = new Map<string, Handler>();
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
			handlers.set(event, handler);
			return { remove: async () => void handlers.delete(event) };
		}),
	};
	const emit = (event: string, data: Record<string, unknown> = {}) =>
		handlers.get(event)?.(data);
	return { plugin, emit, handlers };
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
		const { plugin, handlers } = fakePlugin();
		const recorder = new NativeRecorder(plugin);
		await recorder.attach();
		expect(handlers.size).toBeGreaterThan(0);
		recorder.dispose();
		await Promise.resolve();
		expect(handlers.size).toBe(0);
	});
});

describe("userKeyOf", () => {
	it("is stable per account and ignores case and spaces", async () => {
		const a = await userKeyOf("Dat@inet.vn");
		expect(a).toMatch(/^[0-9a-f]{24}$/);
		expect(await userKeyOf(" dat@inet.vn ")).toBe(a);
		expect(await userKeyOf("other@inet.vn")).not.toBe(a);
	});
});
