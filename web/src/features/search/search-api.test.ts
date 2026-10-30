import { afterEach, describe, expect, it } from "vitest";
import { setActiveTranslator } from "../../i18n/active-translator";
import { VIETNAMESE } from "../../i18n/i18n-context";
import { zhCN } from "../../i18n/messages/zh-CN/dictionary";
import { createTranslator } from "../../i18n/translate";
import { fakeFetch, jsonResponse } from "../../test/fake-fetch";
import {
	type AskReply,
	answerParts,
	askScribe,
	searchTranscripts,
} from "./search-api";

const REPLY: AskReply = {
	answer: "Chốt 5 triệu [1][2]. Hạn [9] thứ Sáu.",
	citations: [
		{
			n: 1,
			key: "f:a",
			title: "Họp",
			createdAt: null,
			at: 61,
			text: "Chốt giá.",
		},
		{
			n: 2,
			key: "f:a",
			title: "Họp",
			createdAt: null,
			at: 75,
			text: "5 triệu.",
		},
	],
};

afterEach(() => {
	setActiveTranslator(VIETNAMESE);
});

describe("search API", () => {
	it("sends the query, the scope and the limit", async () => {
		const fake = fakeFetch({
			"GET /api/search": jsonResponse({ results: [], total: 0 }),
		});
		await searchTranscripts(
			"hợp đồng",
			{ key: "s:abc", limit: 5 },
			undefined,
			fake.fetcher,
		);
		expect(fake.calls[0]?.url).toBe(
			"/api/search?q=h%E1%BB%A3p+%C4%91%E1%BB%93ng&key=s%3Aabc&limit=5",
		);
	});

	it("asks with or without a transcript", async () => {
		const fake = fakeFetch({ "POST /api/ask": jsonResponse(REPLY) });
		await askScribe("Giá?", undefined, fake.fetcher);
		await askScribe("Giá?", "f:a", fake.fetcher);
		expect(fake.calls.map((c) => c.body)).toEqual([
			{ question: "Giá?", lang: "vi" },
			{ question: "Giá?", key: "f:a", lang: "vi" },
		]);
	});

	it("asks for the answer in the language on screen", async () => {
		setActiveTranslator(createTranslator("zh-CN", zhCN));
		const fake = fakeFetch({ "POST /api/ask": jsonResponse(REPLY) });
		await askScribe("价格？", "f:a", fake.fetcher);
		expect(fake.calls[0]?.body).toEqual({
			question: "价格？",
			key: "f:a",
			lang: "zh-CN",
		});
	});

	it("cuts the answer at its citations; unknown numbers stay text", () => {
		const parts = answerParts(REPLY);
		expect(
			parts.map((p) => (p.kind === "text" ? p.text : p.citation.n)),
		).toEqual(["Chốt 5 triệu ", 1, 2, ". Hạn [9] thứ Sáu."]);
		expect(answerParts({ answer: "Không tìm thấy.", citations: [] })).toEqual([
			{ kind: "text", text: "Không tìm thấy." },
		]);
	});
});
