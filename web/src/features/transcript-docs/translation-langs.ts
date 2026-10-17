import type { TranslationLang } from "./transcript-docs-api";

/** Languages a transcript can be translated into, in display order. */
export const TRANSLATION_LANGS: readonly TranslationLang[] = ["en", "vi"];

/** Dictionary key of each language's name ("Tiếng Anh", "Tiếng Việt"). */
export const TRANSLATION_LANG_LABEL = {
	en: "transcript.translate.languages.en",
	vi: "transcript.translate.languages.vi",
} as const satisfies Record<TranslationLang, string>;
