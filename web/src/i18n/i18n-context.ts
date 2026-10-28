/* Copied from localman-next apps/web/src/i18n/i18n-context.ts. Scribe
   change: no `setLocale` (the footer language switch stores the shared
   choice and `I18nProvider` follows it) and the default is Vietnamese. */
import { createContext, useContext } from "react";
import { DEFAULT_LOCALE } from "./locales";
import { vi } from "./messages/vi/dictionary";
import { type Translate, type Translator, createTranslator } from "./translate";

export type I18nValue = Translator;

/**
 * Vietnamese: the default language, and the one of components rendered
 * outside `<I18nProvider>` (unit tests of a single component).
 */
export const VIETNAMESE: I18nValue = createTranslator(DEFAULT_LOCALE, vi);

export const I18nContext = createContext<I18nValue>(VIETNAMESE);

/** Current language, `t()`, number and date formatting. */
export function useI18n(): I18nValue {
	return useContext(I18nContext);
}

/** `t(key, params?)` of the current language. */
export function useT(): Translate {
	return useContext(I18nContext).t;
}
