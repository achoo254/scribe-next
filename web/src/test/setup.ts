import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";

/*
 * jsdom has no PointerEvent; without it Testing Library's pointer events
 * lose their coordinates. Pointer events are mouse events plus pointer
 * fields the components do not read.
 */
if (
	typeof globalThis.PointerEvent === "undefined" &&
	typeof MouseEvent !== "undefined"
) {
	class TestPointerEvent extends MouseEvent {}
	globalThis.PointerEvent = TestPointerEvent as typeof PointerEvent;
}

afterEach(() => {
	cleanup();
});
