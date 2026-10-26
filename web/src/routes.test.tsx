import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ReactNode } from "react";
import { RouterProvider, createMemoryRouter } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fakeFetch, jsonResponse } from "./test/fake-fetch";
import { TestProviders } from "./test/test-providers";

const auth = vi.hoisted(() => ({
	signedIn: true,
	canTranscribe: true,
	splitThrows: false,
	stateMounts: 0,
}));

vi.mock("./auth/auth-context", () => ({
	useAuth: () => ({
		ready: true,
		user: auth.signedIn
			? { email: "a@b.c", canTranscribe: auth.canTranscribe }
			: null,
		logout: async () => undefined,
	}),
}));
vi.mock("./components/usage-card", () => ({ UsageCard: () => null }));
vi.mock("./layout/signed-in-state", async () => {
	const { useEffect } = await import("react");
	return {
		// Counts mounts: a page error must not remount the recording state.
		SignedInState: function SignedInState({
			children,
		}: { children: ReactNode }) {
			useEffect(() => {
				auth.stateMounts += 1;
			}, []);
			return children;
		},
	};
});
vi.mock("./pages/login-page", () => ({ LoginPage: () => <p>login</p> }));
vi.mock("./pages/split/split-page", () => ({
	SplitPage: () => {
		if (auth.splitThrows) throw new Error("split page broke");
		return <p>split page</p>;
	},
}));
vi.mock("./pages/record/record-page", async () => {
	const { useOpenTranscript } = await import("./layout/transcript-nav");
	return {
		// Stands in for "Mở transcript" in the recordings list.
		RecordPage: function RecordPage() {
			const open = useOpenTranscript();
			return (
				<>
					<p>record page</p>
					{open ? (
						<button type="button" onClick={() => open("s:aaaaaaaa-1111")}>
							Mở transcript
						</button>
					) : null}
				</>
			);
		},
	};
});
vi.mock("./pages/transcript/transcript-list", () => ({
	TranscriptList: () => <p>transcript list</p>,
}));
vi.mock("./pages/transcript/transcript-reader", () => ({
	TranscriptReader: ({
		transcriptKey,
		onBack,
	}: { transcriptKey: string; onBack: () => void }) => (
		<>
			<p>{`reader ${transcriptKey}`}</p>
			<button type="button" onClick={onBack}>
				Danh sách
			</button>
		</>
	),
}));

const { appRoutes } = await import("./routes");

function renderAt(path: string) {
	const router = createMemoryRouter(appRoutes, { initialEntries: [path] });
	render(
		<TestProviders>
			<RouterProvider router={router} />
		</TestProviders>,
	);
	return router;
}

beforeEach(() => {
	// The account menu reads the chat.hoandat.com link.
	vi.stubGlobal(
		"fetch",
		fakeFetch({ "GET /api/chat/link": jsonResponse({ linked: false }) })
			.fetcher,
	);
});

afterEach(() => {
	vi.unstubAllGlobals();
	auth.signedIn = true;
	auth.canTranscribe = true;
	auth.splitThrows = false;
	auth.stateMounts = 0;
});

describe("app routes", () => {
	it("sends a signed-in user from / and unknown paths to /record", async () => {
		const router = renderAt("/");
		expect(await screen.findByText("record page")).toBeInTheDocument();
		expect(router.state.location.pathname).toBe("/record");
		await act(() => router.navigate("/nowhere"));
		await waitFor(() => expect(router.state.location.pathname).toBe("/record"));
		expect(screen.getByText("record page")).toBeInTheDocument();
	});

	it("opens /transcripts/:key with the decoded key (keys contain ':')", async () => {
		const router = renderAt("/transcripts/s%3Aaaaaaaaa-1111");
		expect(
			await screen.findByText("reader s:aaaaaaaa-1111"),
		).toBeInTheDocument();
		await userEvent.click(screen.getByRole("button", { name: "Danh sách" }));
		expect(await screen.findByText("transcript list")).toBeInTheDocument();
		expect(router.state.location.pathname).toBe("/transcripts");
	});

	it("opens a transcript from another page by its URL", async () => {
		const router = renderAt("/record");
		await userEvent.click(
			await screen.findByRole("button", { name: "Mở transcript" }),
		);
		expect(
			await screen.findByText("reader s:aaaaaaaa-1111"),
		).toBeInTheDocument();
		expect(router.state.location.pathname).toBe(
			"/transcripts/s%3Aaaaaaaaa-1111",
		);
	});

	it("shows a page error below the shell without remounting the recording state", async () => {
		vi.spyOn(console, "error").mockImplementation(() => undefined);
		auth.splitThrows = true;
		const router = renderAt("/record");
		expect(await screen.findByText("record page")).toBeInTheDocument();
		await act(() => router.navigate("/split"));
		expect(await screen.findByText("Trang này gặp lỗi")).toBeInTheDocument();
		expect(
			screen.getAllByRole("link", { name: "Ghi âm" }).length,
		).toBeGreaterThan(0);
		auth.splitThrows = false;
		await act(() => router.navigate("/record"));
		expect(await screen.findByText("record page")).toBeInTheDocument();
		expect(auth.stateMounts).toBe(1);
	});

	it("shows a visitor the landing page at /, whose calls to action open /login", async () => {
		auth.signedIn = false;
		const router = renderAt("/");
		expect(
			await screen.findByRole("heading", {
				level: 1,
				name: "Nói. Nhận bản chép lời.",
			}),
		).toBeInTheDocument();
		expect(router.state.location.pathname).toBe("/");
		expect(auth.stateMounts).toBe(0);
		expect(screen.getByRole("link", { name: "Đăng nhập" })).toHaveAttribute(
			"href",
			"/login",
		);
		const starts = screen.getAllByRole("link", { name: "Bắt đầu miễn phí" });
		expect(starts).toHaveLength(2);
		for (const start of starts) expect(start).toHaveAttribute("href", "/login");
		expect(screen.getByRole("link", { name: "Xem tính năng" })).toHaveAttribute(
			"href",
			"#features",
		);
		expect(
			screen.getByRole("link", { name: "Một app của hoandat.com" }),
		).toHaveAttribute("href", "https://hoandat.com");
		// The footer is the account app's: language, privacy policy, terms.
		const footer = document.querySelector<HTMLElement>(
			'[data-id="public-footer"]',
		);
		if (!footer) throw new Error("no public footer");
		expect(
			within(footer).getByRole("link", { name: "Tiếng Việt" }),
		).toHaveAttribute("aria-current", "true");
		expect(
			within(footer).getByRole("link", { name: "Quyền riêng tư" }),
		).toHaveAttribute("href", "/privacy");
		expect(
			within(footer).getByRole("link", { name: "Điều khoản" }),
		).toHaveAttribute("href", "/terms");
		await userEvent.click(screen.getByRole("link", { name: "Đăng nhập" }));
		expect(await screen.findByText("login")).toBeInTheDocument();
		expect(router.state.location.pathname).toBe("/login");
	});

	it("keeps the sign-in card at /login for a visitor", async () => {
		auth.signedIn = false;
		const router = renderAt("/login");
		expect(await screen.findByText("login")).toBeInTheDocument();
		expect(router.state.location.pathname).toBe("/login");
		expect(screen.queryByRole("heading", { level: 1 })).toBeNull();
	});

	it("sends a signed-in user from /login to the app", async () => {
		const router = renderAt("/login");
		expect(await screen.findByText("record page")).toBeInTheDocument();
		expect(router.state.location.pathname).toBe("/record");
		expect(screen.queryByText("Nói. Nhận bản chép lời.")).toBeNull();
	});

	it("takes a visitor in the iPhone or Android app from / straight to /login", async () => {
		auth.signedIn = false;
		vi.stubGlobal("Capacitor", { isNativePlatform: () => true });
		const router = renderAt("/");
		expect(await screen.findByText("login")).toBeInTheDocument();
		expect(router.state.location.pathname).toBe("/login");
	});

	it("opens the legal pages without signing in, every other page goes to /login", async () => {
		auth.signedIn = false;
		const router = renderAt("/privacy");
		expect(
			await screen.findByRole("heading", {
				level: 1,
				name: "Chính sách quyền riêng tư",
			}),
		).toBeInTheDocument();
		expect(auth.stateMounts).toBe(0);
		await userEvent.click(
			screen.getAllByRole("link", { name: "Điều khoản" })[0],
		);
		expect(
			await screen.findByRole("heading", {
				level: 1,
				name: "Điều khoản sử dụng",
			}),
		).toBeInTheDocument();
		expect(router.state.location.pathname).toBe("/terms");
		await act(() => router.navigate("/record"));
		expect(await screen.findByText("login")).toBeInTheDocument();
		expect(router.state.location.pathname).toBe("/login");
	});

	it("shows the legal pages inside the shell when signed in, keeping the recording state", async () => {
		const router = renderAt("/record");
		expect(await screen.findByText("record page")).toBeInTheDocument();
		await act(() => router.navigate("/terms"));
		expect(
			await screen.findByRole("heading", {
				level: 1,
				name: "Điều khoản sử dụng",
			}),
		).toBeInTheDocument();
		expect(
			screen.getAllByRole("link", { name: "Ghi âm" }).length,
		).toBeGreaterThan(0);
		await act(() => router.navigate("/record"));
		expect(await screen.findByText("record page")).toBeInTheDocument();
		expect(auth.stateMounts).toBe(1);
	});

	it("has no Transcript pages for users who may not transcribe", async () => {
		auth.canTranscribe = false;
		const router = renderAt("/transcripts/s%3Aaaaaaaaa-1111");
		expect(await screen.findByText("record page")).toBeInTheDocument();
		await waitFor(() => expect(router.state.location.pathname).toBe("/record"));
		expect(screen.queryByRole("button", { name: "Mở transcript" })).toBeNull();
		expect(screen.queryByRole("link", { name: "Lời đã chép" })).toBeNull();
		await act(() => router.navigate("/transcripts"));
		await waitFor(() => expect(router.state.location.pathname).toBe("/record"));
	});
});
