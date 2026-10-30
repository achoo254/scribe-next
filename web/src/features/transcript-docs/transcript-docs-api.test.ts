import { afterEach, describe, expect, it } from "vitest";
import { setActiveTranslator } from "../../i18n/active-translator";
import { VIETNAMESE } from "../../i18n/i18n-context";
import { zhCN } from "../../i18n/messages/zh-CN/dictionary";
import { createTranslator } from "../../i18n/translate";
import { ApiError } from "../../lib/api";
import { fakeFetch, jsonResponse } from "../../test/fake-fetch";
import {
	addBookmark,
	createSummary,
	deleteBookmark,
	deleteTranscript,
	fetchTranscriptDoc,
	fetchTranscriptStatus,
	listTranscriptDocs,
	renameTranscript,
	saveSegmentTexts,
	shareSummary,
	startTranslation,
	transcriptKey,
} from "./transcript-docs-api";

const KEY = "s:6ad09d56-a5e5";
const PATH = "/api/transcript-docs/s%3A6ad09d56-a5e5";

afterEach(() => {
	setActiveTranslator(VIETNAMESE);
});

describe("transcript-docs api", () => {
	it("builds the key the backend groups by", () => {
		expect(transcriptKey({ session: "6ad09d56-a5e5" })).toBe(KEY);
		expect(transcriptKey({ fileId: "65f0a1b2c3d4e5f60718293a" })).toBe(
			"f:65f0a1b2c3d4e5f60718293a",
		);
	});

	it("lists, reads and polls with the encoded key", async () => {
		const { fetcher, calls } = fakeFetch({
			"GET /api/transcript-docs": jsonResponse({ items: [{ key: KEY }] }),
			[`GET ${PATH}`]: jsonResponse({ key: KEY, segments: [] }),
			[`GET ${PATH}/status`]: jsonResponse({ items: [{ fileId: "a" }] }),
		});
		expect(await listTranscriptDocs(fetcher)).toEqual([{ key: KEY }]);
		expect((await fetchTranscriptDoc(KEY, fetcher)).key).toBe(KEY);
		expect(await fetchTranscriptStatus(KEY, fetcher)).toEqual([
			{ fileId: "a" },
		]);
		expect(calls.every((call) => call.credentials === "include")).toBe(true);
	});

	it("sends an empty title as null (back to the default name)", async () => {
		const { fetcher, calls } = fakeFetch({
			[`PATCH ${PATH}`]: jsonResponse({ title: null }),
		});
		expect(await renameTranscript(KEY, "   ", fetcher)).toBeNull();
		expect(calls[0]?.body).toEqual({ title: null });
	});

	it("saves one segment's texts with the run they replace", async () => {
		const { fetcher, calls } = fakeFetch({
			[`PUT ${PATH}/segments/f1`]: jsonResponse({ ok: true }),
		});
		await saveSegmentTexts(KEY, "f1", "run-1", ["Một.", "Hai."], fetcher);
		expect(calls[0]?.body).toEqual({ runId: "run-1", texts: ["Một.", "Hai."] });
	});

	it("surfaces the backend's conflict message", async () => {
		const { fetcher } = fakeFetch({
			[`PUT ${PATH}/segments/f1`]: jsonResponse(
				{ error: "đoạn này vừa được chép lại, tải lại để sửa tiếp" },
				409,
			),
		});
		const error = await saveSegmentTexts(KEY, "f1", "old", [], fetcher).catch(
			(cause: unknown) => cause,
		);
		expect(error).toBeInstanceOf(ApiError);
		expect((error as ApiError).status).toBe(409);
	});

	it("deletes the transcript", async () => {
		const { fetcher, calls } = fakeFetch({
			[`DELETE ${PATH}`]: jsonResponse({ ok: true }),
		});
		await deleteTranscript(KEY, fetcher);
		expect(calls[0]?.method).toBe("DELETE");
	});

	it("summarises, shares, and adds and deletes bookmarks under the doc", async () => {
		const { fetcher, calls } = fakeFetch({
			[`POST ${PATH}/summary`]: jsonResponse({ summary: "Tóm tắt." }),
			[`POST ${PATH}/share`]: jsonResponse({ ok: true }),
			[`POST ${PATH}/bookmarks`]: jsonResponse({
				id: "65a1b2c3d4e5f60718293a4b",
				at: 12.5,
				label: null,
			}),
			[`DELETE ${PATH}/bookmarks/b1`]: jsonResponse({ ok: true }),
		});
		expect((await createSummary(KEY, fetcher)).summary).toBe("Tóm tắt.");
		await shareSummary(KEY, fetcher);
		expect(
			await addBookmark(KEY, 12.5, "65a1b2c3d4e5f60718293a4b", fetcher),
		).toEqual({
			id: "65a1b2c3d4e5f60718293a4b",
			at: 12.5,
			label: null,
		});
		await deleteBookmark(KEY, "b1", fetcher);
		expect(calls.map((call) => call.method)).toEqual([
			"POST",
			"POST",
			"POST",
			"DELETE",
		]);
		expect(calls[0]?.body).toEqual({ lang: "vi" });
		expect(calls[2]?.body).toEqual({
			id: "65a1b2c3d4e5f60718293a4b",
			at: 12.5,
		});
	});

	it("asks for minutes and translations in Chinese", async () => {
		setActiveTranslator(createTranslator("zh-CN", zhCN));
		const { fetcher, calls } = fakeFetch({
			[`POST ${PATH}/summary`]: jsonResponse({
				summary: "摘要。",
				lang: "zh-CN",
			}),
			[`POST ${PATH}/translation`]: jsonResponse({
				lang: "zh-CN",
				status: "running",
				done: 0,
				total: 3,
				error: null,
			}),
		});
		expect((await createSummary(KEY, fetcher)).lang).toBe("zh-CN");
		expect((await startTranslation(KEY, "zh-CN", fetcher)).lang).toBe("zh-CN");
		expect(calls.map((call) => call.body)).toEqual([
			{ lang: "zh-CN" },
			{ lang: "zh-CN" },
		]);
	});

	it("keeps the code of a refused share", async () => {
		const { fetcher } = fakeFetch({
			[`POST ${PATH}/share`]: jsonResponse(
				{ error: "Chưa liên kết chat.hoandat.com.", code: "CHAT_NOT_LINKED" },
				409,
			),
		});
		await expect(shareSummary(KEY, fetcher)).rejects.toMatchObject({
			status: 409,
			code: "CHAT_NOT_LINKED",
		});
	});
});
