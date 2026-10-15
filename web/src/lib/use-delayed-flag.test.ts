import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useDelayedFlag } from "./use-delayed-flag";

function renderFlag(initial: boolean) {
	return renderHook(({ active }) => useDelayedFlag(active), {
		initialProps: { active: initial },
	});
}

function advance(ms: number) {
	act(() => {
		vi.advanceTimersByTime(ms);
	});
}

describe("useDelayedFlag", () => {
	beforeEach(() => vi.useFakeTimers());
	afterEach(() => vi.useRealTimers());

	it("shows nothing for a load shorter than the delay", () => {
		const { result, rerender } = renderFlag(true);
		advance(149);
		expect(result.current).toBe(false);
		rerender({ active: false });
		advance(1000);
		expect(result.current).toBe(false);
	});

	it("turns on after 150 ms and stays on for 300 ms after a quick finish", () => {
		const { result, rerender } = renderFlag(true);
		advance(150);
		expect(result.current).toBe(true);
		advance(50);
		rerender({ active: false });
		advance(249);
		expect(result.current).toBe(true);
		advance(1);
		expect(result.current).toBe(false);
	});

	it("turns off at once when the load already lasted past the minimum", () => {
		const { result, rerender } = renderFlag(true);
		advance(150 + 500);
		expect(result.current).toBe(true);
		rerender({ active: false });
		advance(0);
		expect(result.current).toBe(false);
	});

	it("does not blink while active toggles on and off quickly", () => {
		const { result, rerender } = renderFlag(false);
		for (let i = 0; i < 10; i += 1) {
			rerender({ active: true });
			advance(100);
			rerender({ active: false });
			advance(20);
			expect(result.current).toBe(false);
		}
	});

	it("stays on without a gap when active returns during the hold", () => {
		const { result, rerender } = renderFlag(true);
		advance(150);
		rerender({ active: false });
		advance(100);
		rerender({ active: true });
		advance(1000);
		expect(result.current).toBe(true);
		rerender({ active: false });
		advance(0);
		expect(result.current).toBe(false);
	});

	it("honours custom delay and minimum duration", () => {
		const { result, rerender } = renderHook(
			({ active }) => useDelayedFlag(active, { delay: 0, minDuration: 50 }),
			{ initialProps: { active: true } },
		);
		advance(0);
		expect(result.current).toBe(true);
		rerender({ active: false });
		advance(49);
		expect(result.current).toBe(true);
		advance(1);
		expect(result.current).toBe(false);
	});
});
