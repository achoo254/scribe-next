// @vitest-environment node
import { describe, expect, it } from "vitest";
import { findLiteralViolations, sourceKindOf } from "./scan-literals.js";

const rules = (source: string, file = "a.tsx") =>
	findLiteralViolations(source, file).map((v) => [v.line, v.rule, v.match]);

describe("findLiteralViolations", () => {
	it("flags hex colours in scripts and stylesheets with line numbers", () => {
		expect(rules('const a = 1;\nconst c = "#6e56cf";')).toEqual([
			[2, "hex-color", "#6e56cf"],
		]);
		expect(rules(".x { color: #FFF; }", "a.css")).toEqual([
			[1, "hex-color", "#FFF"],
		]);
		expect(rules('className="bg-[#ff000080]"')).toEqual([
			[1, "hex-color", "#ff000080"],
		]);
	});

	it("ignores non-colour hashes", () => {
		expect(rules('<a href="#app">x</a>')).toEqual([]);
		expect(rules('getElementById("app"); const s = "&#123;";')).toEqual([]);
		expect(rules('const id = "#main-content";')).toEqual([]);
	});

	it("flags colour functions", () => {
		expect(rules(".x { background: rgb(0 0 0 / 0.5); }", "a.css")).toEqual([
			[1, "color-function", "rgb("],
		]);
		expect(rules('style={{ color: "hsla(0, 0%, 0%, .5)" }}')).toEqual([
			[1, "color-function", "hsla("],
		]);
	});

	it("flags --color-* theme names, which are not CSS variables", () => {
		expect(
			rules(
				'"bg-[color-mix(in_srgb,var(--color-primary)_10%,var(--color-bg))]"',
			),
		).toEqual([
			[1, "theme-color-var", "--color-primary"],
			[1, "theme-color-var", "--color-bg"],
		]);
		expect(rules('className="text-(--color-muted)"')).toEqual([
			[1, "theme-color-var", "--color-muted"],
		]);
		expect(
			rules("@theme inline { --color-bg: var(--c-bg); }", "a.css"),
		).toEqual([]);
		expect(
			rules('"bg-[color-mix(in_srgb,var(--c-primary)_10%,var(--c-bg))]"'),
		).toEqual([]);
	});

	it("flags px spacing in CSS but not other px properties", () => {
		expect(rules(".x { padding: 12px 4px; gap: 6px }", "a.css")).toEqual([
			[1, "spacing-px", "padding: 12px"],
			[1, "spacing-px", "padding: 4px"],
			[1, "spacing-px", "gap: 6px"],
		]);
		expect(
			rules(
				".x { width: 440px; border: 1px solid; outline-offset: 2px; font-size: 13px }",
				"a.css",
			),
		).toEqual([]);
	});

	it("allows the documented px values in spacing", () => {
		expect(rules(".x { margin-top: -1px; inset: 0px 2px }", "a.css")).toEqual(
			[],
		);
	});

	it("flags px in React style spacing and Tailwind arbitrary spacing", () => {
		expect(rules('<div style={{ paddingTop: "12px" }} />')).toEqual([
			[1, "spacing-px", "paddingTop: 12px"],
		]);
		expect(
			rules('className="p-[12px] md:-mt-[3px] gap-x-[6px] w-[440px]"'),
		).toEqual([
			[1, "spacing-px", "p-[12px]"],
			[1, "spacing-px", "-mt-[3px]"],
			[1, "spacing-px", "gap-x-[6px]"],
		]);
	});

	it("accepts token-based spacing", () => {
		expect(
			rules(
				'className="p-4 gap-1.5 pt-(--modal-offset-top) px-[var(--gutter)]"',
			),
		).toEqual([]);
		expect(rules(".x { padding: var(--space-3) }", "a.css")).toEqual([]);
	});

	it("skips lines with an ignore marker only when a reason is given", () => {
		expect(
			rules('const c = "#fff"; // check-tokens-ignore: test fixture'),
		).toEqual([]);
		expect(rules('const c = "#fff"; // check-tokens-ignore:')).toEqual([
			[1, "hex-color", "#fff"],
		]);
	});
});

describe("sourceKindOf", () => {
	it("treats TS/JS as scripts and everything else as stylesheets", () => {
		expect(sourceKindOf("x.tsx")).toBe("script");
		expect(sourceKindOf("x.ts")).toBe("script");
		expect(sourceKindOf("x.css")).toBe("stylesheet");
		expect(sourceKindOf("index.html")).toBe("stylesheet");
	});
});
