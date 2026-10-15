import { act, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { PanelProgress } from "./panel-progress";

const bar = () => document.querySelector('[data-id="panel-progress"]');
const content = () => document.querySelector("[data-content]")?.parentElement;

function advance(ms: number) {
	act(() => {
		vi.advanceTimersByTime(ms);
	});
}

function renderPanel(active: boolean, dim = false) {
	const view = render(
		<PanelProgress active={active} dim={dim} data-id="panel" aria-busy={active}>
			<p data-content="">Rows</p>
		</PanelProgress>,
	);
	const rerender = (next: boolean, nextDim = dim) =>
		view.rerender(
			<PanelProgress
				active={next}
				dim={nextDim}
				data-id="panel"
				aria-busy={next}
			>
				<p data-content="">Rows</p>
			</PanelProgress>,
		);
	return { ...view, rerender };
}

describe("PanelProgress", () => {
	beforeEach(() => vi.useFakeTimers());
	afterEach(() => vi.useRealTimers());

	it("keeps its content and shows no bar while idle", () => {
		renderPanel(false);
		expect(document.querySelector('[data-id="panel"]')).toHaveClass("relative");
		expect(document.querySelector("[data-content]")).toHaveTextContent("Rows");
		expect(bar()).toBeNull();
	});

	it("shows nothing for a refetch faster than 150 ms", () => {
		const { rerender } = renderPanel(true);
		advance(100);
		rerender(false);
		advance(500);
		expect(bar()).toBeNull();
	});

	it("shows a hidden bar at the top on a slow refetch, held 300 ms", () => {
		const { rerender } = renderPanel(true);
		advance(150);
		expect(bar()).toHaveAttribute("aria-hidden", "true");
		expect(bar()).toHaveClass("absolute", "top-0", "h-1");
		expect(document.querySelector('[data-id="panel"]')).toHaveAttribute(
			"aria-busy",
			"true",
		);
		rerender(false);
		advance(299);
		expect(bar()).not.toBeNull();
		advance(1);
		expect(bar()).toBeNull();
	});

	it("dims the content only while the bar shows and dim is set", () => {
		const { rerender } = renderPanel(true, true);
		expect(content()).not.toHaveClass("opacity-60");
		advance(150);
		expect(content()).toHaveClass("opacity-60");
		rerender(true, false);
		expect(content()).not.toHaveClass("opacity-60");
	});

	it("keeps a static full-width segment under reduced motion", () => {
		renderPanel(true);
		advance(150);
		const segment = bar()?.firstElementChild;
		expect(segment).toHaveClass(
			"w-full",
			"motion-safe:w-1/3",
			"motion-safe:animate-progress-sweep",
		);
		expect(segment).not.toHaveClass("animate-progress-sweep");
	});
});
