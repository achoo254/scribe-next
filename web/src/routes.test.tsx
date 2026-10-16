import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ReactNode } from "react";
import { RouterProvider, createMemoryRouter } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fakeFetch, jsonResponse } from "./test/fake-fetch";
import { TestProviders } from "./test/test-providers";

const auth = vi.hoisted(() => ({
	canTranscribe: true,
	splitThrows: false,
	stateMounts: 0,
}));

vi.mock("./auth/auth-context", () => ({
	useAuth: () => ({
		ready: true,
		user: { email: "a@b.c", canTranscribe: auth.canTranscribe },
		logout: async () => undefined,
	}),
}));
vi.mock("./components/quota-bar", () => ({ QuotaBar: () => null }));
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
	auth.canTranscribe = true;
	auth.splitThrows = false;
	auth.stateMounts = 0;
});

describe("app routes", () => {
	it("sends / and unknown paths to /record", async () => {
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
