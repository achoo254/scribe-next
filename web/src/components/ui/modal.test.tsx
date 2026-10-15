import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { Button } from "./button";
import { Modal, ModalClose, type ModalSize } from "./modal";
import { ModalFooter } from "./modal-footer";

function renderModal(size?: ModalSize, onOpenChange = vi.fn()) {
	render(
		<Modal
			size={size}
			title="Tạo resource"
			description="Đặt tên cho resource mới."
			onOpenChange={onOpenChange}
			trigger={<Button>Mở</Button>}
			footer={
				<ModalFooter>
					<ModalClose asChild>
						<Button>Huỷ</Button>
					</ModalClose>
					<Button variant="primary">Lưu</Button>
				</ModalFooter>
			}
		>
			<p>Nội dung</p>
		</Modal>,
	);
	return { onOpenChange };
}

describe("Modal", () => {
	it("opens from its trigger with title and description wired", async () => {
		renderModal();
		expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
		await userEvent.click(screen.getByRole("button", { name: "Mở" }));
		const dialog = screen.getByRole("dialog", { name: "Tạo resource" });
		expect(dialog).toHaveAccessibleDescription("Đặt tên cho resource mới.");
		expect(dialog).toHaveAttribute("data-state", "open");
		expect(dialog).toHaveClass(
			"rounded-lg",
			"data-[state=open]:animate-modal-in",
		);
	});

	it.each([
		["sm", "max-w-sm"],
		["md", "max-w-md"],
		["lg", "max-w-xl"],
		["xl", "max-w-3xl"],
	] as const)("size %s applies %s", async (size, expected) => {
		renderModal(size);
		await userEvent.click(screen.getByRole("button", { name: "Mở" }));
		const dialog = screen.getByRole("dialog");
		expect(dialog).toHaveAttribute("data-size", size);
		expect(dialog).toHaveClass(expected);
	});

	it("closes via the close icon button", async () => {
		const { onOpenChange } = renderModal();
		await userEvent.click(screen.getByRole("button", { name: "Mở" }));
		await userEvent.click(screen.getByRole("button", { name: "Đóng" }));
		await waitFor(() =>
			expect(screen.queryByRole("dialog")).not.toBeInTheDocument(),
		);
		expect(onOpenChange).toHaveBeenLastCalledWith(false);
	});

	it("closes via Escape and via a footer ModalClose", async () => {
		renderModal();
		await userEvent.click(screen.getByRole("button", { name: "Mở" }));
		await userEvent.keyboard("{Escape}");
		await waitFor(() =>
			expect(screen.queryByRole("dialog")).not.toBeInTheDocument(),
		);
		await userEvent.click(screen.getByRole("button", { name: "Mở" }));
		await userEvent.click(screen.getByRole("button", { name: "Huỷ" }));
		await waitFor(() =>
			expect(screen.queryByRole("dialog")).not.toBeInTheDocument(),
		);
	});

	it("renders a sticky two-column footer", async () => {
		renderModal();
		await userEvent.click(screen.getByRole("button", { name: "Mở" }));
		const footer = screen.getByRole("button", { name: "Lưu" }).parentElement;
		expect(footer).toHaveAttribute("data-layout", "split");
		expect(footer).toHaveClass("sticky", "bottom-0", "grid-cols-2");
	});
});

describe("ModalFooter", () => {
	it("switches to one column for a single action", () => {
		render(
			<ModalFooter>
				<Button>OK</Button>
			</ModalFooter>,
		);
		const footer = screen.getByRole("button", { name: "OK" }).parentElement;
		expect(footer).toHaveAttribute("data-layout", "single");
		expect(footer).toHaveClass("grid-cols-1");
	});
});
