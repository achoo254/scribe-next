import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useOpenTranscript } from "./transcript-nav";

const auth = vi.hoisted(() => ({ canTranscribe: false }));

vi.mock("../auth/auth-context", () => ({
	useAuth: () => ({
		user: { email: "a@b.c", canTranscribe: auth.canTranscribe },
	}),
}));
vi.mock("../components/quota-bar", () => ({ QuotaBar: () => null }));
vi.mock("./app-top-bar", () => ({ AppTopBar: () => null }));
vi.mock("../pages/split/split-page", () => ({ SplitPage: () => null }));
vi.mock("../pages/record/record-page", () => ({
	// Stands in for "Mở transcript" in the recordings list.
	RecordPage: function RecordPage() {
		const open = useOpenTranscript();
		return open ? (
			<button type="button" onClick={() => open("s:aaaaaaaa-1111")}>
				Mở transcript
			</button>
		) : null;
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

afterEach(() => {
	auth.canTranscribe = false;
});

describe("AppShell", () => {
	it("hides the Transcript tab from users who may not transcribe", () => {
		render(<AppShell />);
		expect(screen.queryByRole("tab", { name: /Transcript/ })).toBeNull();
		expect(screen.queryByRole("button", { name: "Mở transcript" })).toBeNull();
	});

	it("shows the tab and opens a transcript from another tab", async () => {
		auth.canTranscribe = true;
		render(<AppShell />);
		const tab = screen.getByRole("tab", { name: /Transcript/ });
		expect(tab).not.toBeDisabled();
		expect(
			screen.getByText("transcript active=false open=-"),
		).toBeInTheDocument();

		await userEvent.click(
			screen.getByRole("button", { name: "Mở transcript" }),
		);
		expect(tab).toHaveAttribute("aria-selected", "true");
		expect(
			screen.getByText("transcript active=true open=s:aaaaaaaa-1111"),
		).toBeInTheDocument();
	});
});
