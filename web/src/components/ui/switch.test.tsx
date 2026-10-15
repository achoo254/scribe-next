import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { Switch } from "./switch";

describe("Switch", () => {
	it("reports the requested state and leaves applying it to the owner", async () => {
		const onCheckedChange = vi.fn();
		render(
			<Switch
				checked={false}
				aria-label="Bật GET /users"
				onCheckedChange={onCheckedChange}
			/>,
		);
		const toggle = screen.getByRole("switch", { name: "Bật GET /users" });
		expect(toggle).toHaveAttribute("aria-checked", "false");
		await userEvent.click(toggle);
		expect(onCheckedChange).toHaveBeenCalledWith(true);
		expect(toggle).toHaveAttribute("aria-checked", "false");
	});

	it("does nothing while disabled", async () => {
		const onCheckedChange = vi.fn();
		render(
			<Switch
				checked
				disabled
				aria-label="Bật"
				onCheckedChange={onCheckedChange}
			/>,
		);
		await userEvent.click(screen.getByRole("switch", { name: "Bật" }));
		expect(onCheckedChange).not.toHaveBeenCalled();
	});
});
