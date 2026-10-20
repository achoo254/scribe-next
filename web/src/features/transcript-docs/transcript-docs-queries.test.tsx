import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { jsonResponse } from "../../test/fake-fetch";
import { TestProviders } from "../../test/test-providers";
import type {
	TranscriptTranslation,
	TranslationProgress,
} from "./transcript-docs-api";
import { useStartTranslation, useTranslation } from "./transcript-docs-queries";

const KEY = "s:s1";

const RUNNING: TranslationProgress = {
	lang: "en",
	status: "running",
	done: 0,
	total: 4,
	error: null,
};

function translationOf(progress: TranslationProgress): TranscriptTranslation {
	return {
		...progress,
		model: null,
		updatedAt: null,
		outdated: 0,
		missing: 0,
		segments: [],
	};
}

afterEach(() => {
	vi.unstubAllGlobals();
});

describe("useStartTranslation", () => {
	it("shows the run even when the language's first load was answered before it started", async () => {
		const loads: string[] = [];
		let answerFirstLoad: (response: Response) => void = () => undefined;
		const fetcher = vi.fn(
			async (input: RequestInfo | URL, init: RequestInit = {}) => {
				const url = String(input);
				if (init.method === "POST") return jsonResponse(RUNNING, 202);
				if (url.includes("/translation/status")) return jsonResponse(RUNNING);
				if (url.includes("/translation?")) {
					loads.push(url);
					if (loads.length > 1) return jsonResponse(translationOf(RUNNING));
					return new Promise<Response>((resolve) => {
						answerFirstLoad = resolve;
					});
				}
				throw new Error(`Unexpected request ${init.method ?? "GET"} ${url}`);
			},
		);
		vi.stubGlobal("fetch", fetcher);

		const { result } = renderHook(
			() => ({
				shown: useTranslation(KEY, "en"),
				start: useStartTranslation(KEY),
			}),
			{ wrapper: TestProviders },
		);
		await waitFor(() => expect(loads).toHaveLength(1));
		await act(() => result.current.start.mutateAsync("en"));
		// The server read the translation before the POST had started the run.
		act(() =>
			answerFirstLoad(
				jsonResponse(translationOf({ ...RUNNING, status: "none", total: 0 })),
			),
		);

		await waitFor(() =>
			expect(result.current.shown.progress?.status).toBe("running"),
		);
		expect(loads).toHaveLength(2);
	});
});
