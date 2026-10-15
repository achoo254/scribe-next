import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { Alert } from "./alert";
import { Badge } from "./badge";

describe("Badge", () => {
	it("renders mono uppercase text with neutral tone by default", () => {
		render(<Badge>Nháp</Badge>);
		const badge = screen.getByText("Nháp");
		expect(badge).toHaveAttribute("data-tone", "neutral");
		expect(badge).toHaveClass("font-mono", "uppercase", "tint-soft");
	});

	it("uses the *-text token for semantic soft labels", () => {
		render(<Badge tone="success">Đang chạy</Badge>);
		const badge = screen.getByText("Đang chạy");
		expect(badge).toHaveAttribute("data-tone", "success");
		expect(badge.className).toContain("[--tone-fg:var(--c-success-text)]");
		expect(badge.className).toContain("[--tone:var(--c-success)]");
	});
});

describe("Alert", () => {
	it("announces warnings assertively and info politely", () => {
		render(
			<>
				<Alert tone="warning" title="Sắp hết hạn mức">
					9/10 resource
				</Alert>
				<Alert tone="info">Mẹo</Alert>
			</>,
		);
		expect(screen.getByRole("alert")).toHaveTextContent("Sắp hết hạn mức");
		expect(screen.getByRole("status")).toHaveTextContent("Mẹo");
	});
});
