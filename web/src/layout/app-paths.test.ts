import { describe, expect, it } from "vitest";
import { parseStartAt, searchPath, transcriptPath } from "./app-paths";

describe("app paths", () => {
	it("opens a transcript at a second", () => {
		expect(transcriptPath("s:abc")).toBe("/transcripts/s%3Aabc");
		expect(transcriptPath("s:abc", 61.46)).toBe("/transcripts/s%3Aabc?t=61.5");
		expect(transcriptPath("f:x", -1)).toBe("/transcripts/f%3Ax");
		expect(parseStartAt("61.5")).toBe(61.5);
		expect(parseStartAt("0")).toBe(0);
		expect(parseStartAt("abc")).toBeUndefined();
		expect(parseStartAt("-3")).toBeUndefined();
		expect(parseStartAt(null)).toBeUndefined();
	});

	it("puts the query of the search page in the URL", () => {
		expect(searchPath("  hợp đồng ")).toBe(
			"/search?q=h%E1%BB%A3p+%C4%91%E1%BB%93ng",
		);
		expect(searchPath("  ")).toBe("/search");
	});
});
