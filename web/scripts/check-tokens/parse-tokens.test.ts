// @vitest-environment node
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import {
	diffDarkBlocks,
	parseTokenBlocks,
	resolveThemes,
} from "./parse-tokens.js";

const SAMPLE = `
/* header with { braces } inside a comment */
:root { --c-bg: #fff; --c-text: #111; --radius-md: 12px; }
@media (min-width:768px) { :root { --gutter: 24px } }
@media (prefers-color-scheme:dark) {
  :root:not([data-theme='light']) { --c-bg: #000; --c-text: #eee; }
}
:root[data-theme="dark"] { --c-bg: #000; --c-text: #eee }
`;

describe("parseTokenBlocks", () => {
	it("records custom properties with their selector context", () => {
		const blocks = parseTokenBlocks(SAMPLE);
		expect(blocks.map((b) => b.context)).toEqual([
			[":root"],
			["@media (min-width: 768px)", ":root"],
			[
				"@media (prefers-color-scheme: dark)",
				':root:not([data-theme="light"])',
			],
			[':root[data-theme="dark"]'],
		]);
		expect(blocks[0]?.tokens.get("--radius-md")).toBe("12px");
	});

	it("rejects unbalanced input", () => {
		expect(() => parseTokenBlocks(":root { --a: 1;")).toThrow(/unclosed/);
	});
});

describe("resolveThemes", () => {
	it("layers dark overrides on top of light and ignores responsive blocks", () => {
		const themes = resolveThemes(SAMPLE);
		expect(themes.light.get("--c-bg")).toBe("#fff");
		expect(themes.light.has("--gutter")).toBe(false);
		expect(themes.dark.get("--c-bg")).toBe("#000");
		expect(themes.dark.get("--radius-md")).toBe("12px");
		expect(diffDarkBlocks(themes)).toEqual([]);
	});

	it("reports drift between the two dark blocks", () => {
		const drifted = SAMPLE.replace(
			':root[data-theme="dark"] { --c-bg: #000;',
			':root[data-theme="dark"] { --c-bg: #010101;',
		);
		expect(diffDarkBlocks(resolveThemes(drifted))).toEqual([
			"--c-bg: media=#000 forced=#010101",
		]);
	});

	it("parses the real tokens.css with identical dark blocks", () => {
		const css = readFileSync(
			fileURLToPath(new URL("../../src/styles/tokens.css", import.meta.url)),
			"utf8",
		);
		const themes = resolveThemes(css);
		expect(themes.light.get("--c-primary")).toBe("#6550b9");
		expect(themes.dark.get("--c-primary")).toBe("#baa7ff");
		expect(diffDarkBlocks(themes)).toEqual([]);
	});
});
