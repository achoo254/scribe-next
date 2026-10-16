import { describe, expect, it } from "vitest";
import {
	highlightParts,
	matchRanges,
	normalizeText,
	queryWords,
} from "./normalize";

describe("normalizeText", () => {
	it("matches the backend: no marks, đ → d, lower case, one space", () => {
		// Same cases as `search::norm` of the backend.
		expect(normalizeText("Hợp đồng")).toBe("hop dong");
		expect(normalizeText("ĐỒNG Ý ký HỢP ĐỒNG!")).toBe("dong y ky hop dong");
		expect(normalizeText("  Thứ   Sáu,\n15/10 — chốt giá 5.000đ ")).toBe(
			"thu sau 15 10 chot gia 5 000d",
		);
		expect(normalizeText("hội".normalize("NFD"))).toBe("hoi");
		expect(normalizeText("…!?")).toBe("");
		expect(queryWords(" hop  ĐỒNG ")).toEqual(["hop", "dong"]);
	});
});

describe("matchRanges", () => {
	it("marks the words in a row in the original text, the last as a prefix", () => {
		const text = "Ký Hợp đồng thuê nhà, hợp đồng mới.";
		expect(matchRanges(text, "hop dong")).toEqual([
			[3, 11],
			[22, 30],
		]);
		expect(matchRanges(text, "hợp đ")).toEqual([
			[3, 8],
			[22, 27],
		]);
		expect(
			highlightParts("Chốt hợp đồng.", "hop dong").map((p) => p.match),
		).toEqual([false, true, false]);
	});

	it("falls back to each word when they are not in a row", () => {
		const text = "Thuê nhà theo hợp đồng.";
		expect(matchRanges(text, "nha hop")).toEqual([
			[5, 8],
			[14, 17],
		]);
		expect(matchRanges(text, "gia")).toEqual([]);
		expect(matchRanges(text, "  ")).toEqual([]);
		expect(highlightParts("Không khớp.", "gia")).toEqual([
			{ text: "Không khớp.", match: false },
		]);
	});

	it("keeps offsets right for characters outside the basic plane", () => {
		expect(matchRanges("🎤 Hợp đồng", "hop")).toEqual([[3, 6]]);
	});
});
