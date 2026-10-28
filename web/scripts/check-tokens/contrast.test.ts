// @vitest-environment node
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import {
	apcaLc,
	evaluatePairs,
	mixSrgb,
	parseHexColor,
	wcagRatio,
} from "./contrast.js";
import { resolveThemes } from "./parse-tokens.js";
import { TOKENS_FILE } from "./tokens-file.js";

describe("colour maths", () => {
	it("parses short and long hex", () => {
		expect(parseHexColor("#fff")).toEqual([255, 255, 255]);
		expect(parseHexColor("#6E56CF")).toEqual([110, 86, 207]);
		expect(() => parseHexColor("rgb(0 0 0)")).toThrow();
		expect(() => parseHexColor("#00000080")).toThrow();
	});

	it("computes WCAG ratios", () => {
		expect(wcagRatio([0, 0, 0], [255, 255, 255])).toBeCloseTo(21, 5);
		expect(wcagRatio([255, 255, 255], [255, 255, 255])).toBeCloseTo(1, 5);
		// Radix slate-11 on white, documented as ≥4.5:1.
		expect(
			wcagRatio(parseHexColor("#60646c"), [255, 255, 255]),
		).toBeGreaterThan(4.5);
	});

	it("mixes like color-mix(in srgb)", () => {
		expect(mixSrgb([0, 0, 0], [255, 255, 255], 8)).toEqual([235, 235, 235]);
		expect(mixSrgb([200, 100, 0], [0, 0, 0], 50)).toEqual([100, 50, 0]);
	});

	it("reports APCA polarity", () => {
		expect(apcaLc([0, 0, 0], [255, 255, 255])).toBeGreaterThan(100);
		expect(apcaLc([255, 255, 255], [0, 0, 0])).toBeLessThan(-100);
	});
});

describe("evaluatePairs", () => {
	it("fails a pair under 4.5:1 and resolves var() aliases", () => {
		const tokens = new Map([
			["--c-text", "#777777"],
			["--c-surface", "#ffffff"],
			["--c-alias", "var(--c-text)"],
		]);
		const [result] = evaluatePairs("test", tokens, [
			{ fg: "--c-alias", bg: "--c-surface", kind: "text" },
		]);
		expect(result?.pass).toBe(false);
		expect(result?.ratio).toBeCloseTo(4.48, 2);
	});

	it("passes every main pair of the real tokens in light and dark", () => {
		const css = readFileSync(TOKENS_FILE, "utf8");
		const themes = resolveThemes(css);
		const failures = [
			...evaluatePairs("light", themes.light),
			...evaluatePairs("dark", themes.dark),
		].filter((r) => !r.pass);
		expect(failures).toEqual([]);
	});
});
