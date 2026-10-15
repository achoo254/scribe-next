import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { MeterBar } from "./meter-bar";

describe("MeterBar", () => {
	it("exposes the rounded value and fills to it", () => {
		render(<MeterBar value={42.6} label="Đang tải lên" />);
		const bar = screen.getByRole("progressbar", { name: "Đang tải lên" });
		expect(bar).toHaveAttribute("aria-valuenow", "43");
		expect(bar.firstElementChild).toHaveStyle({ width: "42.6%" });
		expect(bar.firstElementChild).toHaveClass("bg-primary");
	});

	it("clamps and switches tone", () => {
		render(
			<MeterBar value={140} label="Dung lượng" role="meter" tone="error" />,
		);
		const bar = screen.getByRole("meter", { name: "Dung lượng" });
		expect(bar).toHaveAttribute("aria-valuenow", "100");
		expect(bar.firstElementChild).toHaveClass("bg-error");
	});
});
