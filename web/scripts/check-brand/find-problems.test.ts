import { describe, expect, it } from "vitest";
import { findBrandProblems } from "./find-problems.js";

const NAME = "Scribe";

describe("findBrandProblems", () => {
	it("flags the product name as a whole string literal in code", () => {
		const problems = findBrandProblems(
			'const a = "x";\nconst name = "Scribe";\nconst b = `Scribe`;',
			"src/lib/x.ts",
			NAME,
		);
		expect(problems.map((p) => p.line)).toEqual([2, 3]);
	});

	it("leaves sentences, identifiers and other words alone", () => {
		const text =
			'title: "Hỏi Scribe",\nexport function askScribe() {}\nconst key = "scribe-theme";';
		expect(findBrandProblems(text, "src/i18n/x.ts", NAME)).toEqual([]);
	});

	it("skips the literal rule in tests and e2e", () => {
		const text = 'expect(x).toBe("Scribe");';
		expect(findBrandProblems(text, "src/a.test.tsx", NAME)).toEqual([]);
		expect(findBrandProblems(text, "e2e/specs/a.spec.ts", NAME)).toEqual([]);
		expect(findBrandProblems(text, "brand/test/a.ts", NAME)).toEqual([]);
	});

	it("flags the name in HTML, which has %APP_NAME%", () => {
		expect(
			findBrandProblems("<title>Scribe · x</title>", "index.html", NAME),
		).toHaveLength(1);
		expect(
			findBrandProblems("<title>%APP_NAME% · x</title>", "index.html", NAME),
		).toEqual([]);
	});

	it("bans the former name anywhere, in any case", () => {
		const problems = findBrandProblems(
			"// oneScribe web",
			"src/a.test.ts",
			NAME,
		);
		expect(problems).toEqual([
			{ file: "src/a.test.ts", line: 1, message: 'contains "OneScribe"' },
		]);
	});

	it("escapes regex characters of the name", () => {
		expect(findBrandProblems('const a = "A.B";', "x.ts", "A.B")).toHaveLength(
			1,
		);
		expect(findBrandProblems('const a = "AxB";', "x.ts", "A.B")).toEqual([]);
	});
});
