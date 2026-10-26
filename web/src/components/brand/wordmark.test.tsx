import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { BrandMark, Wordmark } from "./wordmark";

describe("Wordmark", () => {
	it("reads Scribe with the 'S' in the primary colour", () => {
		const { container } = render(<Wordmark data-id="brand" />);
		const root = container.firstElementChild;
		expect(root).toHaveTextContent(/^Scribe$/);
		expect(root).toHaveClass("font-mono", "font-bold", "text-fg");
		expect(root).toHaveAttribute("data-id", "brand");
		expect(screen.getByText("S")).toHaveClass("text-primary");
	});
});

describe("BrandMark", () => {
	it("is the decorative app mark image", () => {
		const { container } = render(<BrandMark size={44} />);
		const img = container.querySelector("img");
		expect(img).toHaveAttribute("alt", "");
		expect(img).toHaveAttribute("width", "44");
		expect(img?.getAttribute("src")).toMatch(/scribe-mark-128\.webp$/);
	});
});
