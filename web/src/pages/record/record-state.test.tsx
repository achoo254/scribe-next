import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ReactNode } from "react";
import { Outlet, RouterProvider, createMemoryRouter } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ToastProvider } from "../../components/ui/toast";
import type { RecorderNativePlugin } from "../../features/recorder/native-recorder";
import { fakeFetch, jsonResponse } from "../../test/fake-fetch";
import { RecordUploadProvider } from "./record-upload-state";

type Handler = (data: Record<string, unknown>) => void;

/*
 * The recorders live in `RecordStateProvider`, above the routes: leaving
 * /record and coming back must find the same recorder (no second session,
 * no dispose that would drop the microphone) and, in the iOS app, must not
 * resume the phone's uploads again.
 */

const native = vi.hoisted(() => {
	const handlers = new Map<string, Set<Handler>>();
	interface Status {
		state: "idle" | "starting" | "recording" | "stopping";
		paused?: boolean;
		elapsedS: number;
		positionS?: number;
		sessionId?: string | null;
		uploadsWaiting: number;
		uploadsFailed: number;
		notice: string;
	}
	const idle: Status = {
		state: "idle",
		elapsedS: 0,
		uploadsWaiting: 0,
		uploadsFailed: 0,
		notice: "",
	};
	const plugin = {
		start: vi.fn(async () => ({ sessionId: "s1" })),
		stop: vi.fn(async () => idle),
		pause: vi.fn(async (): Promise<Status> => idle),
		resume: vi.fn(async (): Promise<Status> => idle),
		getStatus: vi.fn(async (): Promise<Status> => idle),
		syncUploads: vi.fn(async () => ({ lostSegments: 0, lostSecondsMax: 0 })),
		signOut: vi.fn(async () => undefined),
		takePendingBookmarks: vi.fn(
			async (): Promise<{ bookmarks: unknown[] }> => ({ bookmarks: [] }),
		),
		ackBookmarks: vi.fn(async () => undefined),
		getTranscript: vi.fn(
			async (): Promise<{
				sessionId: string | null;
				entries: unknown[];
				status: unknown;
			}> => ({ sessionId: null, entries: [], status: {} }),
		),
		addListener: vi.fn(async (event: string, handler: Handler) => {
			const set = handlers.get(event) ?? new Set<Handler>();
			set.add(handler);
			handlers.set(event, set);
			return { remove: async () => void set.delete(handler) };
		}),
	};
	const emit = (event: string, data: Record<string, unknown>) => {
		for (const handler of handlers.get(event) ?? []) handler(data);
	};
	return { plugin, emit, handlers };
});

const browser = vi.hoisted(() => ({ created: 0, disposed: 0 }));

/** The backend's id of the browser recording's file, known once it starts. */
const BROWSER_FILE_ID = vi.hoisted(() => "65f0a1b2c3d4e5f60718293a");

vi.mock("../../features/recorder/native-recorder", async (importOriginal) => {
	const original =
		await importOriginal<
			typeof import("../../features/recorder/native-recorder")
		>();
	const plugin: RecorderNativePlugin = native.plugin;
	return { ...original, NativeRecorderPlugin: plugin };
});

// Counts recorders and disposals; the real one needs a microphone.
vi.mock("../../features/recorder/recorder", () => ({
	Recorder: class {
		private state = {
			recording: false,
			starting: false,
			finalizing: false,
			elapsedS: 0,
			previewStream: null,
			error: "",
			fileId: null as string | null,
		};
		private readonly listeners = new Set<() => void>();
		constructor() {
			browser.created += 1;
		}
		getState = () => this.state;
		subscribe = (listener: () => void) => {
			this.listeners.add(listener);
			return () => this.listeners.delete(listener);
		};
		start = async () => {
			this.state = { ...this.state, recording: true, fileId: BROWSER_FILE_ID };
			for (const listener of this.listeners) listener();
		};
		stop = async () => null;
		position = () => 12.5;
		dispose = () => {
			browser.disposed += 1;
		};
	},
}));

vi.mock("../../auth/auth-context", () => ({
	useAuth: () => ({
		user: { email: "minh.anh@example.com", canTranscribe: true },
		logout: async () => undefined,
	}),
}));

const { BrowserRecordProvider, NativeRecordProvider, useBrowserRecordState } =
	await import("./record-state");
const { NativeRecordCard } = await import("./native-record-card");

function BrowserProbe() {
	const { rec, toggle, splitEnabled, setSplitEnabled } =
		useBrowserRecordState();
	return (
		<>
			<p>{rec.recording ? "đang ghi" : "chưa ghi"}</p>
			<p>{splitEnabled ? "cắt sau khi ghi" : "không cắt"}</p>
			<button type="button" onClick={() => void toggle()}>
				Bắt đầu
			</button>
			<button type="button" onClick={() => setSplitEnabled(true)}>
				Bật cắt
			</button>
		</>
	);
}

function BrowserMarkProbe() {
	const { toggle, mark, bookmarks } = useBrowserRecordState();
	return (
		<>
			<p>{bookmarks.marked} mốc</p>
			<button type="button" onClick={() => void toggle()}>
				Bắt đầu
			</button>
			{mark && (
				<button type="button" onClick={mark}>
					Đánh dấu
				</button>
			)}
		</>
	);
}

function renderRoutes(
	Provider: (props: { children: ReactNode }) => ReactNode,
	page: ReactNode,
) {
	const queryClient = new QueryClient({
		defaultOptions: { queries: { retry: false } },
	});
	const router = createMemoryRouter(
		[
			{
				path: "/",
				element: (
					<Provider>
						<RecordUploadProvider>
							<Outlet />
						</RecordUploadProvider>
					</Provider>
				),
				children: [
					{ path: "record", element: page },
					{ path: "split", element: <p>trang cắt file</p> },
				],
			},
		],
		{ initialEntries: ["/record"] },
	);
	render(
		<QueryClientProvider client={queryClient}>
			<ToastProvider>
				<RouterProvider router={router} />
			</ToastProvider>
		</QueryClientProvider>,
	);
	return router;
}

async function leaveAndComeBack(
	router: ReturnType<typeof createMemoryRouter>,
): Promise<void> {
	for (const path of ["/split", "/record", "/split", "/record"]) {
		await act(() => router.navigate(path));
	}
}

afterEach(() => {
	window.localStorage.clear();
	browser.created = 0;
	browser.disposed = 0;
	native.handlers.clear();
	vi.clearAllMocks();
	vi.unstubAllGlobals();
});

describe("RecordStateProvider (browser)", () => {
	it("keeps the one recorder and the options while /record is left", async () => {
		const router = renderRoutes(BrowserRecordProvider, <BrowserProbe />);
		await userEvent.click(screen.getByRole("button", { name: "Bật cắt" }));
		await userEvent.click(screen.getByRole("button", { name: "Bắt đầu" }));
		expect(await screen.findByText("đang ghi")).toBeInTheDocument();

		await leaveAndComeBack(router);
		expect(screen.getByText("đang ghi")).toBeInTheDocument();
		expect(screen.getByText("cắt sau khi ghi")).toBeInTheDocument();
		expect(browser.created).toBe(1);
		expect(browser.disposed).toBe(0);
	});

	it("shows a recording that saved itself when the user's stop finds nothing to save", async () => {
		const refresh = vi.spyOn(QueryClient.prototype, "invalidateQueries");
		renderRoutes(BrowserRecordProvider, <BrowserProbe />);
		await userEvent.click(screen.getByRole("button", { name: "Bắt đầu" }));
		expect(await screen.findByText("đang ghi")).toBeInTheDocument();
		refresh.mockClear();

		// The mock's stop() returns null, like a stop racing the storage-full stop.
		await userEvent.click(screen.getByRole("button", { name: "Bắt đầu" }));
		expect(refresh).toHaveBeenCalledWith({ queryKey: ["recordings"] });
		refresh.mockRestore();
	});

	it("bookmarks the browser recording under its file id at the recorder's position", async () => {
		const fake = fakeFetch({
			[`POST /api/transcript-docs/f%3A${BROWSER_FILE_ID}/bookmarks`]:
				jsonResponse({ id: "b1", at: 12.5, label: null }),
		});
		vi.stubGlobal("fetch", fake.fetcher);
		renderRoutes(BrowserRecordProvider, <BrowserMarkProbe />);
		// No recording yet: nothing to bookmark.
		expect(screen.queryByRole("button", { name: "Đánh dấu" })).toBeNull();

		await userEvent.click(screen.getByRole("button", { name: "Bắt đầu" }));
		await userEvent.click(
			await screen.findByRole("button", { name: "Đánh dấu" }),
		);
		expect(await screen.findByText("1 mốc")).toBeInTheDocument();
		await waitFor(() => expect(fake.calls).toHaveLength(1));
		expect(fake.calls[0]?.url).toBe(
			`/api/transcript-docs/f%3A${BROWSER_FILE_ID}/bookmarks`,
		);
		expect(fake.calls[0]?.body).toEqual({
			at: 12.5,
			id: expect.stringMatching(/^[0-9a-f]{24}$/),
		});
	});
});

describe("RecordStateProvider (iOS app)", () => {
	it("keeps Swift's state and the segment length across visits, syncing uploads once", async () => {
		const router = renderRoutes(NativeRecordProvider, <NativeRecordCard />);
		await waitFor(() => expect(native.plugin.syncUploads).toHaveBeenCalled());
		// Every plugin event the page uses is listened to (state … quotaFull, the
		// native bookmark pull's bookmarkAdded and state, and transcript), once.
		await waitFor(() =>
			expect(native.plugin.addListener).toHaveBeenCalledTimes(13),
		);
		await userEvent.click(screen.getByRole("button", { name: "5 phút" }));

		await leaveAndComeBack(router);
		expect(screen.getByRole("button", { name: "5 phút" })).toHaveAttribute(
			"aria-pressed",
			"true",
		);
		await userEvent.click(screen.getByRole("button", { name: "Bắt đầu ghi" }));
		expect(native.plugin.start).toHaveBeenCalledWith({
			userKey: expect.any(String),
			segmentMinutes: 5,
		});
		act(() => {
			native.emit("state", { state: "recording" });
			native.emit("elapsed", { elapsedS: 125 });
		});
		expect(await screen.findByText("Đang ghi — 2:05")).toBeInTheDocument();

		await leaveAndComeBack(router);
		expect(screen.getByText("Đang ghi — 2:05")).toBeInTheDocument();
		expect(native.plugin.syncUploads).toHaveBeenCalledTimes(1);
		// Listeners were attached once, not again per visit.
		expect(native.plugin.addListener).toHaveBeenCalledTimes(13);
	});

	it("pauses and resumes without stopping, when the app build can pause", async () => {
		const idleStatus: Awaited<ReturnType<typeof native.plugin.getStatus>> = {
			state: "idle",
			paused: false,
			elapsedS: 0,
			uploadsWaiting: 0,
			uploadsFailed: 0,
			notice: "",
		};
		const recording = {
			...idleStatus,
			state: "recording" as const,
			elapsedS: 42,
		};
		native.plugin.getStatus.mockResolvedValueOnce(idleStatus);
		native.plugin.pause.mockResolvedValueOnce({ ...recording, paused: true });
		native.plugin.resume.mockResolvedValueOnce(recording);
		renderRoutes(NativeRecordProvider, <NativeRecordCard />);
		await waitFor(() =>
			expect(native.plugin.addListener).toHaveBeenCalledTimes(13),
		);
		expect(screen.queryByRole("button", { name: "Tạm dừng" })).toBeNull();
		await userEvent.click(screen.getByRole("button", { name: "Bắt đầu ghi" }));
		act(() => {
			native.emit("state", { state: "recording" });
			native.emit("elapsed", { elapsedS: 42 });
		});

		await userEvent.click(
			await screen.findByRole("button", { name: "Tạm dừng" }),
		);
		expect(native.plugin.pause).toHaveBeenCalledTimes(1);
		expect(await screen.findByText("Đã tạm dừng — 0:42")).toBeInTheDocument();
		await userEvent.click(screen.getByRole("button", { name: "Ghi tiếp" }));
		expect(native.plugin.resume).toHaveBeenCalledTimes(1);
		expect(await screen.findByText("Đang ghi — 0:42")).toBeInTheDocument();

		// From the lock screen while the page is on screen.
		act(() => native.emit("paused", { paused: true }));
		expect(
			await screen.findByRole("button", { name: "Ghi tiếp" }),
		).toBeInTheDocument();
		expect(native.plugin.stop).not.toHaveBeenCalled();
	});

	it("bookmarks at the session position Swift reports, without stopping", async () => {
		const fake = fakeFetch({
			"POST /api/transcript-docs/s%3As1/bookmarks": [
				jsonResponse({ error: "Mỗi bản ghi có tối đa 200 dấu mốc." }, 409),
				jsonResponse({ id: "b1", at: 75, label: null }),
			],
		});
		vi.stubGlobal("fetch", fake.fetcher);
		renderRoutes(NativeRecordProvider, <NativeRecordCard />);
		await waitFor(() =>
			expect(native.plugin.addListener).toHaveBeenCalledTimes(13),
		);
		await userEvent.click(screen.getByRole("button", { name: "Bắt đầu ghi" }));
		act(() => {
			native.emit("state", { state: "recording" });
			native.emit("elapsed", { elapsedS: 75.4 });
		});
		// Swift: 60 s in the closed first segment plus 15 s into the second.
		const recording: Awaited<ReturnType<typeof native.plugin.getStatus>> = {
			state: "recording",
			elapsedS: 75.4,
			positionS: 75,
			sessionId: "s1",
			uploadsWaiting: 0,
			uploadsFailed: 0,
			notice: "",
		};
		native.plugin.getStatus
			.mockResolvedValueOnce(recording)
			.mockResolvedValueOnce(recording);

		const mark = await screen.findByRole("button", { name: "Đánh dấu" });
		await userEvent.click(mark);
		expect(
			await screen.findByText(
				"1 mốc không lưu được: Mỗi bản ghi có tối đa 200 dấu mốc.",
			),
		).toBeInTheDocument();
		await userEvent.click(mark);
		expect(await screen.findByText("Đã đánh 1 mốc")).toBeInTheDocument();
		const bodies = fake.calls.map((call) => call.body);
		expect(bodies).toHaveLength(2);
		expect(bodies[1]).toMatchObject({ at: 75, id: expect.any(String) });
		expect(native.plugin.stop).not.toHaveBeenCalled();
		expect(screen.getByText("Đang ghi — 1:15")).toBeInTheDocument();
	});

	it("sends bookmarks Swift saved on the lock screen, then lets Swift forget them", async () => {
		const fake = fakeFetch({
			"POST /api/transcript-docs/s%3Aabc/bookmarks": [
				jsonResponse({ id: "0123456789abcdef01234567", at: 42, label: null }),
			],
		});
		vi.stubGlobal("fetch", fake.fetcher);
		native.plugin.takePendingBookmarks.mockResolvedValueOnce({
			bookmarks: [
				{
					id: "0123456789abcdef01234567",
					sessionId: "abc",
					positionS: 42,
					createdAt: "2026-10-17T01:00:00Z",
				},
			],
		});
		renderRoutes(NativeRecordProvider, <NativeRecordCard />);
		await waitFor(() =>
			expect(native.plugin.ackBookmarks).toHaveBeenCalledWith({
				userKey: expect.any(String),
				ids: ["0123456789abcdef01234567"],
			}),
		);
		expect(fake.calls.map((call) => call.body)).toEqual([
			{ at: 42, id: "0123456789abcdef01234567" },
		]);
	});

	it("streams a live transcript only when switched on, and shows it without stopping", async () => {
		renderRoutes(NativeRecordProvider, <NativeRecordCard />);
		await waitFor(() =>
			expect(native.plugin.addListener).toHaveBeenCalledTimes(13),
		);
		const toggle = screen.getByRole("switch", { name: "Chép trực tiếp" });
		expect(toggle).not.toBeChecked();
		await userEvent.click(toggle);
		expect(window.localStorage.getItem("scribe.native.realtime")).toBe("1");
		await userEvent.click(screen.getByRole("button", { name: "Bắt đầu ghi" }));
		expect(native.plugin.start).toHaveBeenCalledWith({
			userKey: expect.any(String),
			segmentMinutes: 1,
			realtime: true,
		});
		act(() => {
			native.emit("state", { state: "recording" });
			native.emit("transcript", {
				sessionId: "s1",
				type: "status",
				connected: true,
				stopped: false,
			});
			native.emit("transcript", {
				sessionId: "s1",
				type: "sentence",
				id: "item_2",
				atS: 65,
				text: "Câu thứ hai.",
			});
			native.emit("transcript", {
				sessionId: "s1",
				type: "sentence",
				id: "item_1",
				atS: 3,
				text: "Câu thứ nhất.",
			});
		});
		expect(await screen.findByText("Đang chép")).toBeInTheDocument();
		const items = screen.getAllByRole("listitem");
		expect(items.map((item) => item.textContent)).toEqual([
			"0:03Câu thứ nhất.",
			"1:05Câu thứ hai.",
		]);

		act(() => {
			native.emit("transcript", {
				sessionId: "s1",
				type: "status",
				connected: false,
				disconnectedAtS: 125,
				error: "Không đủ phút chép lời: tháng này đã dùng 120/120 phút.",
				stopped: false,
			});
		});
		expect(
			screen.getByText("Mất kết nối từ 2:05, đang thử nối lại…"),
		).toBeInTheDocument();
		expect(
			screen.getByText(
				"Không đủ phút chép lời: tháng này đã dùng 120/120 phút.",
			),
		).toBeInTheDocument();
		expect(native.plugin.stop).not.toHaveBeenCalled();

		// Back from the lock screen: Swift's copy replaces the page's.
		native.plugin.getTranscript.mockResolvedValueOnce({
			sessionId: "s1",
			entries: [
				{ id: "item_1", atS: 3, text: "Câu thứ nhất." },
				{ id: "gap-1300", atS: 130, text: "", gapUntilS: 190 },
				{ id: "item_9", atS: 200, text: "Câu lúc khoá máy." },
			],
			status: { connected: true, stopped: false },
		});
		act(() => {
			document.dispatchEvent(new Event("visibilitychange"));
		});
		expect(await screen.findByText("Câu lúc khoá máy.")).toBeInTheDocument();
		expect(screen.queryByText("Câu thứ hai.")).not.toBeInTheDocument();
		expect(
			screen.getByText("2:10–3:10 · mất đoạn này, chờ bản hoàn chỉnh"),
		).toBeInTheDocument();
	});

	it("keeps the live transcript off by default and remembers the last choice", async () => {
		window.localStorage.setItem("scribe.native.realtime", "1");
		renderRoutes(NativeRecordProvider, <NativeRecordCard />);
		const toggle = await screen.findByRole("switch", {
			name: "Chép trực tiếp",
		});
		expect(toggle).toBeChecked();
		await userEvent.click(toggle);
		expect(window.localStorage.getItem("scribe.native.realtime")).toBe("0");
		await userEvent.click(screen.getByRole("button", { name: "Bắt đầu ghi" }));
		expect(native.plugin.start).toHaveBeenCalledWith({
			userKey: expect.any(String),
			segmentMinutes: 1,
		});
		expect(screen.queryByText("Đang chép")).not.toBeInTheDocument();
	});
});
