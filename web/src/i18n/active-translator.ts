/* Copied from localman-next apps/web/src/i18n/active-translator.ts. Scribe
   change: Vietnamese is the default (the only language). */
import { DEFAULT_LOCALE } from "./locales";
import { vi } from "./messages/vi/dictionary";
import { type Translator, createTranslator } from "./translate";

/**
 * The translator of the language on screen, for plain functions that run
 * outside React (error messages). `I18nProvider` keeps it current; without
 * a provider it stays Vietnamese. Components use `useT()` instead, so they
 * re-render when the language changes.
 */
let active: Translator = createTranslator(DEFAULT_LOCALE, vi);

export function setActiveTranslator(translator: Translator): void {
	active = translator;
}

export function activeTranslator(): Translator {
	return active;
}
