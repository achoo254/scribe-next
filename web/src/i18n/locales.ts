/**
 * Languages of the Scribe UI. Stands in for `@localman/shared/constants`
 * in the files copied from localman-next: Vietnamese is the only language
 * and the source of every key, so there is no language switcher yet.
 */
export type Locale = "vi";

export const DEFAULT_LOCALE: Locale = "vi";

/** Every language of the UI, in the order the language switcher lists them. */
export const LOCALES: readonly Locale[] = ["vi"];

/** Each language's name in that language (the switcher's labels). */
export const LOCALE_NAMES: Readonly<Record<Locale, string>> = {
	vi: "Tiếng Việt",
};
