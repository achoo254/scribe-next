import { fireEvent, render } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { AudioPlayer } from "./media-player";

describe("AudioPlayer", () => {
	it("reports the playing time and the end of the file", () => {
		const onTimeUpdate = vi.fn();
		const onEnded = vi.fn();
		const { container } = render(
			<AudioPlayer
				src="/api/files/a"
				durationS={60}
				onTimeUpdate={onTimeUpdate}
				onEnded={onEnded}
			/>,
		);
		const audio = container.querySelector("audio");
		if (!audio) throw new Error("no <audio>");
		Object.defineProperty(audio, "currentTime", {
			value: 12.5,
			writable: true,
		});
		fireEvent.timeUpdate(audio);
		expect(onTimeUpdate).toHaveBeenCalledWith(12.5);
		fireEvent.ended(audio);
		expect(onEnded).toHaveBeenCalledTimes(1);
	});

	it("starts by itself only when asked", () => {
		const { container, rerender } = render(<AudioPlayer src="/api/files/a" />);
		expect(container.querySelector("audio")?.autoplay).toBe(false);
		rerender(<AudioPlayer src="/api/files/b" autoPlay />);
		expect(container.querySelector("audio")?.autoplay).toBe(true);
	});
});
