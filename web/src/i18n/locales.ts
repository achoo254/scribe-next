/**
 * Languages of the Scribe UI, signed in or not: Vietnamese, English and
 * Simplified Chinese, the three the shared shell speaks
 * (@achoo254/app-shell). Scribe passes this list to the shared language
 * switch and preference reader, which otherwise assume vi and en only.
 * Vietnamese is the source of every key (`messages/vi`).
 */
import type { ShellLocale } from "@achoo254/app-shell";

/** Every language of the UI, in the order the language switch lists them. */
export const LOCALES = [
	"vi",
	"en",
	"zh-CN",
] as const satisfies readonly ShellLocale[];

export type Locale = (typeof LOCALES)[number];

export const DEFAULT_LOCALE: Locale = "vi";

/** BCP 47 tag of a language for `Intl` and `toLocale*String`. */
export const INTL_LOCALE: Readonly<Record<Locale, string>> = {
	vi: "vi-VN",
	en: "en-US",
	"zh-CN": "zh-CN",
};
