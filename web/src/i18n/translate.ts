/* Copied from localman-next apps/web/src/i18n/translate.ts. Scribe change:
   only the Vietnamese date format (the only language). */
import type { Locale } from "./locales";
import type {
	MessageKey,
	MessageLeaf,
	Messages,
	ParamValue,
	ParamsArg,
	PluralForms,
} from "./message-types";

/** Dates are shown in the product's timezone, whatever the browser's is. */
export const APP_TIME_ZONE = "Asia/Saigon";

/** Vietnamese dates read `dd/mm/yyyy`. */
const DATE_OPTIONS: Record<Locale, Intl.DateTimeFormatOptions> = {
	vi: {
		day: "2-digit",
		month: "2-digit",
		year: "numeric",
		timeZone: APP_TIME_ZONE,
	},
};

/** `t(key, params?)`: the text of `key` in the current language. */
export type Translate = <K extends MessageKey>(
	key: K,
	...params: ParamsArg<K>
) => string;

export interface Translator {
	locale: Locale;
	t: Translate;
	/** Number with the language's digit grouping ("10,000", "10.000"). */
	formatNumber: (value: number) => string;
	/** Date of an ISO string in Asia/Saigon, or "" when unparsable. */
	formatDate: (iso: string) => string;
}

function isPlural(leaf: unknown): leaf is PluralForms {
	return (
		typeof leaf === "object" &&
		leaf !== null &&
		"other" in leaf &&
		typeof leaf.other === "string"
	);
}

/** The leaf at a dotted path, or undefined when a segment is missing. */
function lookup(messages: Messages, key: string): MessageLeaf | undefined {
	let node: unknown = messages;
	for (const part of key.split(".")) {
		if (typeof node !== "object" || node === null) return undefined;
		node = (node as Record<string, unknown>)[part];
	}
	return typeof node === "string" || isPlural(node) ? node : undefined;
}

export function createTranslator(
	locale: Locale,
	messages: Messages,
): Translator {
	const numbers = new Intl.NumberFormat(locale);
	const dates = new Intl.DateTimeFormat(locale, DATE_OPTIONS[locale]);
	const plurals = new Intl.PluralRules(locale);

	function interpolate(
		text: string,
		params: Readonly<Record<string, ParamValue>>,
	): string {
		return text.replace(/\{(\w+)\}/g, (match, name: string) => {
			const value = params[name];
			if (value === undefined) return match;
			return typeof value === "number" ? numbers.format(value) : value;
		});
	}

	const t: Translate = (key, ...rest) => {
		const params: Readonly<Record<string, ParamValue>> = rest[0] ?? {};
		const leaf = lookup(messages, key);
		// Unreachable with typed keys; shows the key instead of crashing.
		if (leaf === undefined) return key;
		if (typeof leaf === "string") return interpolate(leaf, params);
		const count = typeof params.count === "number" ? params.count : 0;
		const form = leaf[plurals.select(count)] ?? leaf.other;
		return interpolate(form, params);
	};

	return {
		locale,
		t,
		formatNumber: (value) => numbers.format(value),
		formatDate: (iso) => {
			const date = new Date(iso);
			return Number.isNaN(date.getTime()) ? "" : dates.format(date);
		},
	};
}
