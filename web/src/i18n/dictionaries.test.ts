import { describe, expect, it } from "vitest";
import { en } from "./messages/en/dictionary";
import { vi as messages } from "./messages/vi/dictionary";
import { zhCN } from "./messages/zh-CN/dictionary";

/*
 * Vietnamese is the source dictionary: it is checked against the code
 * (every key the code asks for exists, every key is used, no text is
 * empty). English and Chinese are checked against Vietnamese: the same
 * keys, the same placeholders and tags, and text in their own language.
 */

type Leaf = { key: string; texts: string[]; plural: boolean };

/** Every leaf of a dictionary with its text (all forms for a plural). */
function leaves(node: unknown, prefix = ""): Leaf[] {
	if (typeof node === "string") {
		return [{ key: prefix, texts: [node], plural: false }];
	}
	if (typeof node !== "object" || node === null) return [];
	const record = node as Record<string, unknown>;
	if (typeof record.other === "string") {
		const texts = Object.values(record).filter(
			(value): value is string => typeof value === "string",
		);
		return [{ key: prefix, texts, plural: true }];
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

/** Rich-text tags of one text, sorted: `<b>`, `<account>`… */
function tags(text: string): string[] {
	return [...text.matchAll(/<(\w+)>/g)].map((match) => match[1] ?? "").sort();
}

const vietnameseLetter =
	/[àáạảãâầấậẩẫăằắặẳẵèéẹẻẽêềếệểễìíịỉĩòóọỏõôồốộổỗơờớợởỡùúụủũưừứựửữỳýỵỷỹđ]/i;

/**
 * English texts that keep Vietnamese on purpose, with the reason. Keep it
 * short: everything else the user reads is English.
 */
const VIETNAMESE_IN_ENGLISH: ReadonlyArray<{ key: string; reason: string }> = [
	{
		key: "chat.dialog.step3",
		reason:
			"names the bot's button, which the Scribe server writes in Vietnamese",
	},
	{
		key: "search.page.placeholder",
		reason: "an example search: the transcripts are Vietnamese speech",
	},
];

describe("en dictionary", () => {
	const source = leaves(messages);
	const translated = new Map(leaves(en).map((leaf) => [leaf.key, leaf]));

	it("has exactly the keys of Vietnamese", () => {
		expect([...translated.keys()].sort()).toEqual(
			source.map((leaf) => leaf.key).sort(),
		);
		expect(source.length).toBeGreaterThan(500);
	});

	it("keeps the placeholders and tags of Vietnamese in every text", () => {
		for (const leaf of source) {
			const [first = ""] = leaf.texts;
			for (const text of translated.get(leaf.key)?.texts ?? []) {
				expect(text.trim(), leaf.key).not.toBe("");
				expect(placeholders(text), leaf.key).toEqual(placeholders(first));
				expect(tags(text), leaf.key).toEqual(tags(first));
			}
		}
	});

	it("gives a plural one/other pair where Vietnamese counts", () => {
		for (const leaf of source.filter((leaf) => leaf.plural)) {
			const english = translated.get(leaf.key);
			expect(english?.plural, leaf.key).toBe(true);
			expect(english?.texts.length, leaf.key).toBe(2);
		}
	});

	it("is English except the listed texts", () => {
		const allowed = new Set(VIETNAMESE_IN_ENGLISH.map((entry) => entry.key));
		const vietnamese = [...translated.values()]
			.filter((leaf) => leaf.texts.some((text) => vietnameseLetter.test(text)))
			.map((leaf) => leaf.key);
		expect(vietnamese.filter((key) => !allowed.has(key))).toEqual([]);
		// Every exception is still needed and explained.
		for (const entry of VIETNAMESE_IN_ENGLISH) {
			expect(vietnamese, entry.key).toContain(entry.key);
			expect(entry.reason.trim(), entry.key).not.toBe("");
		}
	});

	it("translates rather than copies the Vietnamese", () => {
		const copied = source
			.filter((leaf) => leaf.texts.some((text) => vietnameseLetter.test(text)))
			.filter(
				(leaf) =>
					JSON.stringify(translated.get(leaf.key)?.texts) ===
					JSON.stringify(leaf.texts),
			)
			.map((leaf) => leaf.key);
		expect(copied).toEqual([]);
	});
});

/** A Han character: every Chinese text has at least one. */
const hanCharacter = /\p{Script=Han}/u;

/**
 * Chinese texts without a Han character, with the reason: brand names and
 * the English summary of the legal pages. Keep it short.
 */
const NOT_CHINESE: ReadonlyArray<{ key: string; reason: string }> = [
	{ key: "keys.provider.elevenlabs.name", reason: "a provider's brand name" },
	{ key: "keys.provider.openai.name", reason: "a provider's brand name" },
	{ key: "keys.provider.gemini.name", reason: "a provider's brand name" },
	{ key: "keys.provider.anthropic.name", reason: "a provider's brand name" },
	{
		key: "legal.privacy.english",
		reason: "the English summary the Vietnamese page ends with",
	},
	{
		key: "legal.terms.english",
		reason: "the English summary the Vietnamese page ends with",
	},
	{ key: "record.session.badge", reason: "the device name, iPhone" },
];

/**
 * Chinese texts that keep Vietnamese on purpose, with the reason. Keep it
 * short: everything else the user reads is Chinese.
 */
const VIETNAMESE_IN_CHINESE: ReadonlyArray<{ key: string; reason: string }> = [
	{
		key: "chat.dialog.step3",
		reason:
			"names the bot's button, which the Scribe server writes in Vietnamese",
	},
	{
		key: "search.page.placeholder",
		reason: "an example search: the transcripts are Vietnamese speech",
	},
];

describe("zh-CN dictionary", () => {
	const source = leaves(messages);
	const translated = new Map(leaves(zhCN).map((leaf) => [leaf.key, leaf]));

	it("has exactly the keys of Vietnamese", () => {
		expect([...translated.keys()].sort()).toEqual(
			source.map((leaf) => leaf.key).sort(),
		);
	});

	it("keeps the placeholders and tags of Vietnamese in every text", () => {
		for (const leaf of source) {
			const [first = ""] = leaf.texts;
			for (const text of translated.get(leaf.key)?.texts ?? []) {
				expect(text.trim(), leaf.key).not.toBe("");
				expect(placeholders(text), leaf.key).toEqual(placeholders(first));
				expect(tags(text), leaf.key).toEqual(tags(first));
			}
		}
	});

	it("gives only the plural form `other` where Vietnamese counts", () => {
		expect(
			new Intl.PluralRules("zh-CN").resolvedOptions().pluralCategories,
		).toEqual(["other"]);
		for (const leaf of source.filter((leaf) => leaf.plural)) {
			const chinese = translated.get(leaf.key);
			expect(chinese?.plural, leaf.key).toBe(true);
			expect(chinese?.texts.length, leaf.key).toBe(1);
		}
	});

	it("is Chinese except the listed texts", () => {
		const allowed = new Set(NOT_CHINESE.map((entry) => entry.key));
		const notChinese = [...translated.values()]
			.filter((leaf) => leaf.texts.some((text) => !hanCharacter.test(text)))
			.map((leaf) => leaf.key);
		expect(notChinese.filter((key) => !allowed.has(key))).toEqual([]);
		// Every exception is still needed and explained.
		for (const entry of NOT_CHINESE) {
			expect(notChinese, entry.key).toContain(entry.key);
			expect(entry.reason.trim(), entry.key).not.toBe("");
		}
	});

	it("has no Vietnamese except the listed texts", () => {
		const allowed = new Set(VIETNAMESE_IN_CHINESE.map((entry) => entry.key));
		const vietnamese = [...translated.values()]
			.filter((leaf) => leaf.texts.some((text) => vietnameseLetter.test(text)))
			.map((leaf) => leaf.key);
		expect(vietnamese.filter((key) => !allowed.has(key))).toEqual([]);
		for (const entry of VIETNAMESE_IN_CHINESE) {
			expect(vietnamese, entry.key).toContain(entry.key);
			expect(entry.reason.trim(), entry.key).not.toBe("");
		}
	});

	it("keeps the English summaries of the legal pages as they are", () => {
		expect(zhCN.legal.privacy.english).toBe(en.legal.privacy.english);
		expect(zhCN.legal.terms.english).toBe(en.legal.terms.english);
	});

	it("uses Chinese punctuation in Chinese sentences", () => {
		// An ASCII comma, colon, semicolon, question or exclamation mark
		// right after a Han character belongs in its full-width form.
		const asciiAfterHan = /\p{Script=Han}[,:;?!]/u;
		const mixed = [...translated.values()]
			.filter((leaf) => leaf.texts.some((text) => asciiAfterHan.test(text)))
			.map((leaf) => leaf.key);
		expect(mixed).toEqual([]);
	});
});
