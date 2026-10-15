import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { type FormEvent, useState } from "react";
import { describe, expect, it, vi } from "vitest";
import { Button } from "./button";
import { IconButton } from "./icon-button";

/** A form whose submit button spins from the first submit on. */
function PendingForm({ onSubmit }: { onSubmit: () => void }) {
	const [saving, setSaving] = useState(false);
	function submit(event: FormEvent) {
		event.preventDefault();
		onSubmit();
		setSaving(true);
	}
	return (
		<form onSubmit={submit}>
			<label>
				Name
				<input name="name" />
			</label>
			<Button type="submit" variant="primary" loading={saving}>
				Save
			</Button>
		</form>
	);
}

describe("Button loading in a form", () => {
	it("keeps focus on the pressed button and does not fade it", async () => {
		render(<PendingForm onSubmit={vi.fn()} />);
		const save = screen.getByRole("button", { name: "Save" });
		await userEvent.click(save);
		expect(save).toHaveAttribute("aria-busy", "true");
		expect(save).toHaveFocus();
		expect(save).not.toBeDisabled();
	});

	it("submits once whether the user clicks again or presses Enter", async () => {
		const onSubmit = vi.fn();
		render(<PendingForm onSubmit={onSubmit} />);
		const input = screen.getByLabelText("Name");
		await userEvent.type(input, "Shop{Enter}");
		expect(onSubmit).toHaveBeenCalledTimes(1);
		await userEvent.type(input, "{Enter}");
		await userEvent.click(screen.getByRole("button", { name: "Save" }));
		await userEvent.keyboard(" ");
		expect(onSubmit).toHaveBeenCalledTimes(1);
	});

	it("leaves a really disabled button disabled", async () => {
		const onClick = vi.fn();
		render(
			<Button disabled loading onClick={onClick}>
				Save
			</Button>,
		);
		const button = screen.getByRole("button", { name: "Save" });
		expect(button).toBeDisabled();
		expect(button).toHaveClass("disabled:opacity-45");
		await userEvent.click(button);
		expect(onClick).not.toHaveBeenCalled();
	});
});

describe("IconButton loading", () => {
	it("keeps focus and ignores clicks while busy", async () => {
		const onClick = vi.fn();
		const { rerender } = render(
			<IconButton label="Refresh" icon={<svg />} onClick={onClick} />,
		);
		const button = screen.getByRole("button", { name: "Refresh" });
		await userEvent.click(button);
		expect(onClick).toHaveBeenCalledTimes(1);
		rerender(
			<IconButton loading label="Refresh" icon={<svg />} onClick={onClick} />,
		);
		expect(button).toHaveFocus();
		await userEvent.click(button);
		await userEvent.keyboard("{Enter}");
		expect(onClick).toHaveBeenCalledTimes(1);
	});

	it("passes the caller's aria-disabled through when not loading", () => {
		render(<IconButton aria-disabled label="Remove" icon={<svg />} />);
		expect(screen.getByRole("button", { name: "Remove" })).toHaveAttribute(
			"aria-disabled",
			"true",
		);
	});
});
