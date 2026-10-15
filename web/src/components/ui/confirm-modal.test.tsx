import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { ApiError } from "../../lib/api";
import { ConfirmModal } from "./confirm-modal";

function renderConfirm(onConfirm: () => Promise<void>) {
	const onOpenChange = vi.fn();
	render(
		<ConfirmModal
			open
			onOpenChange={onOpenChange}
			title="Xoá project"
			question="Xoá “Demo”?"
			confirmLabel="Xoá project"
			onConfirm={onConfirm}
		>
			Không hoàn tác được.
		</ConfirmModal>,
	);
	return onOpenChange;
}

describe("ConfirmModal", () => {
	it("closes once the action succeeds", async () => {
		const onOpenChange = renderConfirm(async () => {});
		expect(screen.getByRole("dialog")).toHaveTextContent(
			"Xoá “Demo”?Không hoàn tác được.",
		);
		await userEvent.click(screen.getByRole("button", { name: "Xoá project" }));
		await waitFor(() => expect(onOpenChange).toHaveBeenCalledWith(false));
	});

	it("stays open with the error when the action fails", async () => {
		const onOpenChange = renderConfirm(() =>
			Promise.reject(new ApiError(404, "Không tìm thấy.")),
		);
		await userEvent.click(screen.getByRole("button", { name: "Xoá project" }));
		expect(await screen.findByText("Không tìm thấy.")).toBeInTheDocument();
		expect(onOpenChange).not.toHaveBeenCalled();
		expect(screen.getByRole("button", { name: "Xoá project" })).toBeEnabled();
	});
});
