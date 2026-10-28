/* Copied from localman-next apps/web/src/i18n/active-translator.ts. Scribe
   change: Vietnamese is the default. */
import { VIETNAMESE } from "./i18n-context";
import type { Translator } from "./translate";

/**
 * The translator of the language on screen, for plain functions that run
 * outside React (error messages, dates of `lib/format.ts`).
 * `I18nProvider` keeps it current; without a provider it stays Vietnamese.
 * Components use `useT()` instead, so they re-render when the language
 * changes.
 */
let active: Translator = VIETNAMESE;

export function setActiveTranslator(translator: Translator): void {
	active = translator;
}

export function activeTranslator(): Translator {
	return active;
}
