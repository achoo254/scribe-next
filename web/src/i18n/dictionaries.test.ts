import { describe, expect, it } from "vitest";
import { vi as messages } from "./messages/vi/dictionary";

/*
 * Scribe has one dictionary, so instead of comparing translations with a
 * source language (as localman-next does) this checks the dictionary
 * against the code: every key the code asks for exists, every key in the
 * dictionary is used, and no text is empty.
 */

type Leaf = { key: string; texts: string[] };

/** Every leaf of a dictionary with its text (all forms for a plural). */
function leaves(node: unknown, prefix = ""): Leaf[] {
	if (typeof node === "string") return [{ key: prefix, texts: [node] }];
	if (typeof node !== "object" || node === null) return [];
	const record = node as Record<string, unknown>;
	if (typeof record.other === "string") {
		const texts = Object.values(record).filter(
			(value): value is string => typeof value === "string",
		);
		return [{ key: prefix, texts }];
	}
	return Object.entries(record).flatMap(([key, value]) =>
		leaves(value, prefix ? `${prefix}.${key}` : key),
	);
}

/** Placeholders of one text, sorted: `{count}`, `{name}`… */
function placeholders(text: string): string[] {
	const names = [...text.matchAll(/\{(\w+)\}/g)].map((match) => match[1]);
	return [...new Set(names)].sort();
}

/** Source of the app, without tests and without the dictionary itself. */
const SOURCES: Record<string, string> = import.meta.glob(
	["../**/*.{ts,tsx}", "!../**/*.test.{ts,tsx}", "!./messages/**"],
	{ query: "?raw", import: "default", eager: true },
);

const dictionary = leaves(messages);
const dictionaryKeys = new Set(dictionary.map((leaf) => leaf.key));

describe("vi dictionary", () => {
	it("reads the app source", () => {
		expect(Object.keys(SOURCES).length).toBeGreaterThan(20);
	});

	it("has every key the code passes to t()", () => {
		const used = Object.entries(SOURCES).flatMap(([file, text]) =>
			[...text.matchAll(/\bt\(\s*"([\w.]+)"/g)].map((match) => ({
				file,
				key: match[1] ?? "",
			})),
		);
		expect(used.length).toBeGreaterThan(0);
		for (const { file, key } of used) {
			expect(dictionaryKeys.has(key), `${key} in ${file}`).toBe(true);
		}
	});

	it("has no key the code never uses", () => {
		const code = Object.values(SOURCES).join("\n");
		const unused = [...dictionaryKeys].filter(
			(key) => !code.includes(`"${key}"`),
		);
		expect(unused).toEqual([]);
	});

	it("leaves no text empty", () => {
		for (const leaf of dictionary) {
			for (const text of leaf.texts) {
				expect(text.trim(), leaf.key).not.toBe("");
			}
		}
	});

	it("uses the same placeholders in every plural form", () => {
		for (const leaf of dictionary) {
			const [first = "", ...rest] = leaf.texts;
			for (const text of rest) {
				expect(placeholders(text), leaf.key).toEqual(placeholders(first));
			}
		}
	});
});
