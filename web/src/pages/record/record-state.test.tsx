import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ReactNode } from "react";
import { Outlet, RouterProvider, createMemoryRouter } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ToastProvider } from "../../components/ui/toast";
import type { RecorderNativePlugin } from "../../features/recorder/native-recorder";

type Handler = (data: Record<string, unknown>) => void;

/*
 * The recorders live in `RecordStateProvider`, above the routes: leaving
 * /record and coming back must find the same recorder (no second session,
 * no dispose that would drop the microphone) and, in the iOS app, must not
 * resume the phone's uploads again.
 */

const native = vi.hoisted(() => {
	const handlers = new Map<string, Set<Handler>>();
	const idle = {
		state: "idle" as const,
		elapsedS: 0,
		uploadsWaiting: 0,
		uploadsFailed: 0,
		notice: "",
	};
	const plugin = {
		start: vi.fn(async () => ({ sessionId: "s1" })),
		stop: vi.fn(async () => idle),
		getStatus: vi.fn(async () => idle),
		syncUploads: vi.fn(async () => ({ lostSegments: 0, lostSecondsMax: 0 })),
		signOut: vi.fn(async () => undefined),
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
			this.state = { ...this.state, recording: true };
			for (const listener of this.listeners) listener();
		};
		stop = async () => null;
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
						<Outlet />
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
	browser.created = 0;
	browser.disposed = 0;
	native.handlers.clear();
	vi.clearAllMocks();
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
});

describe("RecordStateProvider (iOS app)", () => {
	it("keeps Swift's state and the segment length across visits, syncing uploads once", async () => {
		const router = renderRoutes(NativeRecordProvider, <NativeRecordCard />);
		await waitFor(() => expect(native.plugin.syncUploads).toHaveBeenCalled());
		// Every plugin event is listened to (state … quotaFull), once.
		await waitFor(() =>
			expect(native.plugin.addListener).toHaveBeenCalledTimes(9),
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
		expect(native.plugin.addListener).toHaveBeenCalledTimes(9);
	});
});
