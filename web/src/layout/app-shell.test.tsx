import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useReportSessionActivity } from "./session-activity";
import { useOpenTranscript } from "./transcript-nav";

const auth = vi.hoisted(() => ({
	canTranscribe: false,
	recording: false,
	logout: vi.fn<() => Promise<void>>(async () => undefined),
	stop: vi.fn<() => Promise<void>>(async () => undefined),
}));

vi.mock("../auth/auth-context", () => ({
	useAuth: () => ({
		user: { email: "a@b.c", canTranscribe: auth.canTranscribe },
		logout: auth.logout,
	}),
}));
vi.mock("../components/quota-bar", () => ({ QuotaBar: () => null }));
vi.mock("../pages/split/split-page", () => ({
	SplitPage: ({ active }: { active: boolean }) => (
		<p>{`split active=${active}`}</p>
	),
}));
vi.mock("../pages/record/record-page", () => ({
	// Stands in for "Mở transcript" in the recordings list and for the
	// recorder reporting a running recording.
	RecordPage: function RecordPage() {
		const open = useOpenTranscript();
		useReportSessionActivity("browser-recorder", {
			recording: auth.recording,
			segmentsWaiting: 0,
			fileUploading: false,
			stopRecording: auth.stop,
		});
		return open ? (
			<button type="button" onClick={() => open("s:aaaaaaaa-1111")}>
				Mở transcript
			</button>
		) : (
			<p>record page</p>
		);
	},
}));
vi.mock("../pages/transcript/transcript-page", () => ({
	TranscriptPage: ({
		active,
		openKey,
	}: { active: boolean; openKey: string | null }) => (
		<p>{`transcript active=${active} open=${openKey ?? "-"}`}</p>
	),
}));

const { AppShell } = await import("./app-shell");

function renderShell() {
	render(
		<MemoryRouter>
			<Routes>
				<Route path="/" element={<AppShell />} />
				<Route path="/login" element={<p>trang đăng nhập</p>} />
			</Routes>
		</MemoryRouter>,
	);
}

/** The wide nav of the top bar (the compact row repeats it under 640px). */
function navButton(name: string): HTMLElement {
	const [button] = screen.getAllByRole("button", { name });
	if (!button) throw new Error(`no nav button ${name}`);
	return button;
}

afterEach(() => {
	auth.canTranscribe = false;
	auth.recording = false;
	auth.logout.mockClear();
	auth.stop.mockClear();
});

describe("AppShell", () => {
	it("hides the Transcript page from users who may not transcribe", () => {
		renderShell();
		expect(screen.queryByRole("button", { name: "Transcript" })).toBeNull();
		expect(screen.queryByRole("button", { name: "Mở transcript" })).toBeNull();
		expect(screen.queryByText(/^transcript active/)).toBeNull();
	});

	it("lays out skip link, top bar, main and footer", () => {
		renderShell();
		expect(
			screen.getByRole("link", { name: "Bỏ qua tới nội dung chính" }),
		).toHaveAttribute("href", "#main");
		expect(screen.getByRole("banner")).toBeInTheDocument();
		expect(screen.getByRole("main")).toHaveAttribute("id", "main");
		expect(screen.getByRole("contentinfo")).toBeInTheDocument();
	});

	it("keeps every page mounted while another one is shown", async () => {
		renderShell();
		expect(navButton("Ghi âm")).toHaveAttribute("aria-current", "page");
		await userEvent.click(navButton("Cắt file"));
		expect(navButton("Cắt file")).toHaveAttribute("aria-current", "page");
		expect(screen.getByText("split active=true")).toBeVisible();
		// Hidden, not unmounted: a recording keeps running.
		expect(screen.getByText("record page")).not.toBeVisible();
		await userEvent.click(navButton("Ghi âm"));
		expect(screen.getByText("record page")).toBeVisible();
		expect(screen.getByText("split active=false")).not.toBeVisible();
	});

	it("opens a transcript from another page", async () => {
		auth.canTranscribe = true;
		renderShell();
		expect(
			screen.getByText("transcript active=false open=-"),
		).toBeInTheDocument();
		await userEvent.click(
			screen.getByRole("button", { name: "Mở transcript" }),
		);
		expect(navButton("Transcript")).toHaveAttribute("aria-current", "page");
		expect(
			screen.getByText("transcript active=true open=s:aaaaaaaa-1111"),
		).toBeVisible();
	});

	it("asks before signing out while the record page is recording", async () => {
		auth.recording = true;
		renderShell();
		screen.getByRole("button", { name: "a@b.c · Tài khoản" }).focus();
		await userEvent.keyboard("{Enter}");
		await userEvent.click(
			await screen.findByRole("menuitem", { name: "Đăng xuất" }),
		);
		await userEvent.click(
			await screen.findByRole("button", { name: "Vẫn đăng xuất" }),
		);
		expect(await screen.findByText("trang đăng nhập")).toBeInTheDocument();
		expect(auth.stop).toHaveBeenCalledTimes(1);
		expect(auth.stop.mock.invocationCallOrder[0]).toBeLessThan(
			auth.logout.mock.invocationCallOrder[0] ?? 0,
		);
	});
});
