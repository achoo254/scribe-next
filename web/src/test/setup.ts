import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";
import { clearPreferences } from "./preferences";

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

/*
 * jsdom has no pointer capture; Radix Toast (swipe to dismiss) calls it on
 * pointer down, e.g. when a test presses "Hoàn tác". No element holds a
 * capture in tests.
 */
if (
	typeof Element !== "undefined" &&
	typeof Element.prototype.hasPointerCapture !== "function"
) {
	Element.prototype.hasPointerCapture = () => false;
	Element.prototype.setPointerCapture = () => undefined;
	Element.prototype.releasePointerCapture = () => undefined;
}

/*
 * Copied from localman-next apps/web/src/test/setup.ts: React Router's
 * data router builds a `Request` (Node/undici) with an AbortSignal from
 * jsdom, which undici rejects. Tests never abort navigations, so the
 * test-only Request drops the foreign signal.
 */
const NodeRequest = globalThis.Request;
class TestRequest extends NodeRequest {
	constructor(input: RequestInfo | URL, init?: RequestInit) {
		const { signal: _signal, ...rest } = init ?? {};
		super(input, rest);
	}
}
globalThis.Request = TestRequest;

/*
 * jsdom reports an English browser (en-US). The signed-out pages follow the
 * device's language, so tests run on a Vietnamese one, like most visitors;
 * the English pages are tested by setting `hd_lang`.
 */
if (typeof window !== "undefined") {
	Object.defineProperty(window.navigator, "language", {
		value: "vi-VN",
		configurable: true,
	});
	afterEach(clearPreferences);
}

afterEach(() => {
	cleanup();
});
