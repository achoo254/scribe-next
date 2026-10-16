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
	it("is decorative and coloured by tokens", () => {
		const { container } = render(<BrandMark size={44} />);
		const svg = container.querySelector("svg");
		expect(svg).toHaveAttribute("aria-hidden", "true");
		expect(svg).toHaveAttribute("width", "44");
		expect(container.querySelector(".fill-primary")).not.toBeNull();
		expect(container.querySelector(".fill-accent")).not.toBeNull();
	});
});
