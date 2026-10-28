/**
 * Languages of the Scribe UI, signed in or not: Vietnamese and English, the
 * shared shell's default pair (@achoo254/app-shell). Scribe has no Chinese,
 * so a `zh-CN` choice stored by another app counts as no choice here and
 * the language switch offers these two only. Vietnamese is the source of
 * every key (`messages/vi`).
 */
import {
	DEFAULT_SHELL_LOCALES,
	type DefaultShellLocale,
} from "@achoo254/app-shell";

export type Locale = DefaultShellLocale;

export const DEFAULT_LOCALE: Locale = "vi";

/** Every language of the UI, in the order the language switch lists them. */
export const LOCALES: readonly Locale[] = DEFAULT_SHELL_LOCALES;

/** BCP 47 tag of a language for `Intl` and `toLocale*String`. */
export const INTL_LOCALE: Readonly<Record<Locale, string>> = {
	vi: "vi-VN",
	en: "en-US",
};
