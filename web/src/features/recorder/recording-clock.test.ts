import { describe, expect, it } from "vitest";
import { RecordingClock } from "./recording-clock";

describe("RecordingClock", () => {
	it("counts from the start, however rarely it is read (hidden tab)", () => {
		let now = 1_000;
		const clock = new RecordingClock(() => now);
		expect(clock.seconds()).toBe(0);
		clock.start();
		now += 2_500;
		expect(clock.seconds()).toBe(2.5);
		// A throttled tab reads it once after ten minutes: no ticks lost.
		now += 600_000;
		expect(clock.seconds()).toBe(602.5);
	});

	it("freezes at stop and starts again from zero", () => {
		let now = 0;
		const clock = new RecordingClock(() => now);
		clock.start();
		now = 61_000;
		clock.stop();
		now = 90_000;
		expect(clock.seconds()).toBe(61);
		clock.stop();
		expect(clock.seconds()).toBe(61);
		clock.start();
		now = 93_000;
		expect(clock.seconds()).toBe(3);
	});
});
