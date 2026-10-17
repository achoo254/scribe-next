import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";
import { renderWithProviders } from "../../test/render-with-providers";
import { SpeakerNamesDialog } from "./speaker-names-dialog";

/** The dialog with names that a background poll replaces by an equal new object. */
function Harness({
	onSave,
}: { onSave: (names: Record<string, string>) => Promise<void> }) {
	const [names, setNames] = useState<Record<string, string>>({
		s0: "Người 1",
		s1: "Người 2",
	});
	return (
		<>
			<button type="button" onClick={() => setNames({ ...names })}>
				poll
			</button>
			<SpeakerNamesDialog
				open
				onOpenChange={() => undefined}
				names={names}
				onSave={onSave}
			/>
		</>
	);
}

describe("SpeakerNamesDialog", () => {
	it("keeps what is being typed when the names are read again in the background", async () => {
		const onSave = vi.fn(async () => undefined);
		renderWithProviders(<Harness onSave={onSave} />);
		const user = userEvent.setup();
		const field = screen.getByRole("textbox", { name: "Tên của Người 1" });
		await user.clear(field);
		await user.type(field, "Lan");
		// The modal makes the rest of the page inert: trigger the poll directly.
		screen.getByText("poll").click();
		expect(
			screen.getByRole("textbox", { name: "Tên của Người 1" }),
		).toHaveValue("Lan");
		await user.click(screen.getByRole("button", { name: "Lưu tên" }));
		expect(onSave).toHaveBeenCalledWith({ s0: "Lan", s1: "Người 2" });
	});
});
