import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { Skeleton, SkeletonRegion } from "./skeleton";

describe("Skeleton", () => {
	it("draws a hidden block sized by className, shimmering only with motion allowed", () => {
		const { container } = render(<Skeleton className="h-6 w-40" />);
		const block = container.firstElementChild;
		expect(block).toHaveAttribute("aria-hidden", "true");
		expect(block).toHaveAttribute("data-skeleton", "block");
		expect(block).toHaveClass(
			"skeleton",
			"motion-safe:skeleton-shimmer",
			"rounded-md",
			"h-6",
			"w-40",
		);
		// Without motion-safe the shimmer would also run under reduced motion.
		expect(block).not.toHaveClass("skeleton-shimmer");
	});

	it("draws a circle", () => {
		const { container } = render(
			<Skeleton variant="circle" className="size-11" />,
		);
		expect(container.firstElementChild).toHaveClass("rounded-full", "size-11");
	});

	it("draws n text lines with a shorter last line", () => {
		const { container } = render(<Skeleton variant="text" lines={3} />);
		const text = container.firstElementChild;
		expect(text).toHaveAttribute("aria-hidden", "true");
		const lines = [...(text?.children ?? [])];
		expect(lines).toHaveLength(3);
		expect(lines[0]).toHaveClass("w-full", "motion-safe:skeleton-shimmer");
		expect(lines[2]).toHaveClass("w-3/5");
	});

	it("keeps a single text line full width", () => {
		const { container } = render(<Skeleton variant="text" lines={1} />);
		expect(container.firstElementChild?.children[0]).toHaveClass("w-full");
	});
});

describe("SkeletonRegion", () => {
	it("marks the area busy and announces the default label once", () => {
		render(
			<SkeletonRegion data-id="list-loading" className="grid gap-3">
				<Skeleton className="h-16" />
				<Skeleton className="h-16" />
			</SkeletonRegion>,
		);
		const statuses = screen.getAllByRole("status");
		expect(statuses).toHaveLength(1);
		expect(statuses[0]).toHaveTextContent("Đang tải…");
		expect(statuses[0]).toHaveClass("sr-only");
		const region = document.querySelector('[data-id="list-loading"]');
		expect(region).toHaveAttribute("aria-busy", "true");
		expect(region).toHaveClass("grid", "gap-3");
		// The status sits beside the busy area, not inside it.
		expect(region?.contains(statuses[0] ?? null)).toBe(false);
	});

	it("uses a custom label", () => {
		render(
			<SkeletonRegion label="Loading projects…">
				<Skeleton className="h-4" />
			</SkeletonRegion>,
		);
		expect(screen.getByRole("status")).toHaveTextContent("Loading projects…");
	});
});
