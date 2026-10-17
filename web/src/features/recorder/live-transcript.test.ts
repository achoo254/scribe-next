import { describe, expect, it, vi } from "vitest";
import {
	EMPTY_LIVE,
	LiveTranscript,
	applyEvent,
	fromSnapshot,
	parseSentence,
} from "./live-transcript";

type Handler = (data: Record<string, unknown>) => void;

describe("live transcript state", () => {
	it("keeps sentences in session order even when they finish out of order", () => {
		let state = applyEvent(EMPTY_LIVE, {
			sessionId: "s1",
			type: "sentence",
			id: "b",
			atS: 12,
			text: "Hai.",
		});
		state = applyEvent(state, {
			sessionId: "s1",
			type: "sentence",
			id: "a",
			atS: 4,
			text: "Một.",
		});
		expect(state.sessionId).toBe("s1");
		expect(state.sentences.map((s) => s.id)).toEqual(["a", "b"]);
	});

	it("replaces a partial with its finished sentence, never the other way", () => {
		let state = applyEvent(EMPTY_LIVE, {
			sessionId: "s1",
			type: "partial",
			id: "a",
			atS: 1,
			text: "Xin",
		});
		expect(state.sentences).toEqual([
			{ id: "a", atS: 1, text: "Xin", partial: true },
		]);
		state = applyEvent(state, {
			sessionId: "s1",
			type: "sentence",
			id: "a",
			atS: 1,
			text: "Xin chào.",
		});
		state = applyEvent(state, {
			sessionId: "s1",
			type: "partial",
			id: "a",
			atS: 1,
			text: "Xin chào. thêm",
		});
		expect(state.sentences).toEqual([{ id: "a", atS: 1, text: "Xin chào." }]);
	});

	it("starts afresh for another session and ignores events without one", () => {
		const first = applyEvent(EMPTY_LIVE, {
			sessionId: "s1",
			type: "sentence",
			id: "a",
			atS: 1,
			text: "Cũ.",
		});
		expect(applyEvent(first, { type: "sentence", id: "x", atS: 1 })).toBe(
			first,
		);
		const next = applyEvent(first, {
			sessionId: "s2",
			type: "status",
			connected: false,
			disconnectedAtS: 30,
			error: "Mất mạng",
		});
		expect(next).toEqual({
			...EMPTY_LIVE,
			sessionId: "s2",
			disconnectedAtS: 30,
			error: "Mất mạng",
		});
	});

	it("reads Swift's snapshot, skipping malformed entries", () => {
		expect(
			fromSnapshot({
				sessionId: "s1",
				entries: [
					{ id: "b", atS: 9, text: "Sau." },
					{ id: "gap-10", atS: 5, text: "", gapUntilS: 8 },
					{ id: "", atS: 1, text: "x" },
					{ id: "c", atS: "?", text: "x" },
					{ id: "d", atS: 2, text: "" },
				],
				status: { connected: true, stopped: true },
			}),
		).toEqual({
			sessionId: "s1",
			sentences: [
				{ id: "gap-10", atS: 5, text: "", gapUntilS: 8 },
				{ id: "b", atS: 9, text: "Sau." },
			],
			connected: true,
			disconnectedAtS: null,
			error: "",
			stopped: true,
		});
		expect(fromSnapshot(null)).toEqual(EMPTY_LIVE);
		expect(parseSentence({ id: "a", atS: Number.NaN, text: "x" })).toBeNull();
	});
});

describe("LiveTranscript", () => {
	function fakePlugin() {
		const handlers = new Set<Handler>();
		return {
			handlers,
			plugin: {
				getTranscript: vi.fn(async () => ({
					sessionId: "s1",
					entries: [{ id: "a", atS: 1, text: "Một." }],
					status: { connected: true },
				})),
				addListener: vi.fn(async (_event: string, handler: Handler) => {
					handlers.add(handler);
					return { remove: async () => void handlers.delete(handler) };
				}),
			},
		};
	}

	it("reads Swift's copy on attach, follows events, and lets go on dispose", async () => {
		const { plugin, handlers } = fakePlugin();
		const live = new LiveTranscript(plugin);
		await live.attach();
		expect(plugin.addListener).toHaveBeenCalledWith(
			"transcript",
			expect.any(Function),
		);
		expect(live.getState().sentences).toHaveLength(1);
		for (const handler of handlers) {
			handler({
				sessionId: "s1",
				type: "sentence",
				id: "b",
				atS: 2,
				text: "Hai.",
			});
		}
		expect(live.getState().sentences.map((s) => s.id)).toEqual(["a", "b"]);
		live.reset();
		expect(live.getState()).toBe(EMPTY_LIVE);
		live.dispose();
		await vi.waitFor(() => expect(handlers.size).toBe(0));
	});

	it("shows nothing when the app build has no live transcript", async () => {
		const { plugin } = fakePlugin();
		plugin.getTranscript.mockRejectedValueOnce(new Error("not implemented"));
		const live = new LiveTranscript(plugin);
		await live.attach();
		expect(live.getState()).toBe(EMPTY_LIVE);
	});
});
