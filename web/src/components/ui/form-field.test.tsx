import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { FormField } from "./form-field";
import { Input } from "./input";

describe("FormField", () => {
	it("links the label to the input", async () => {
		render(
			<FormField label="Tên project">
				<Input />
			</FormField>,
		);
		const input = screen.getByLabelText("Tên project");
		expect(input.tagName).toBe("INPUT");
		await userEvent.click(screen.getByText("Tên project"));
		expect(input).toHaveFocus();
	});

	it("describes the input with hint and shows the error", () => {
		render(
			<FormField
				label="Endpoint"
				hint="Bắt đầu bằng /"
				error="Endpoint đã tồn tại"
			>
				<Input defaultValue="/users" />
			</FormField>,
		);
		const input = screen.getByLabelText("Endpoint");
		expect(input).toHaveAttribute("aria-invalid", "true");
		expect(input).toHaveAccessibleDescription(
			"Bắt đầu bằng / Endpoint đã tồn tại",
		);
		// The error is read once through aria-describedby, not again as a live region.
		expect(screen.getByText("Endpoint đã tồn tại")).not.toHaveAttribute("role");
	});

	it("is valid without an error and marks required controls", () => {
		render(
			<FormField label="Email" required>
				<Input type="email" />
			</FormField>,
		);
		const input = screen.getByLabelText(/Email/);
		expect(input).not.toHaveAttribute("aria-invalid");
		expect(input).toBeRequired();
		expect(input).not.toHaveAccessibleDescription();
	});
	it("keeps explicit control ids and describedby", () => {
		render(
			<>
				<p id="extra">Thêm</p>
				<FormField label="Số phút" hint="Từ 1 trở lên">
					<Input id="minutes" aria-describedby="extra" />
				</FormField>
			</>,
		);
		const input = screen.getByRole("textbox");
		expect(input).toHaveAttribute("id", "minutes");
		expect(input).toHaveAccessibleDescription("Thêm Từ 1 trở lên");
	});
});
