/* Adapted from localman-next apps/web/src/i18n/i18n-provider.tsx. Scribe
   ships one language, so the lazy dictionaries, the saved choice and the
   account sync of the source are left out: Vietnamese is imported
   statically (no chunk for an open tab to miss after a deploy). */
import { type ReactNode, useEffect, useMemo } from "react";
import { setActiveTranslator } from "./active-translator";
import { I18nContext, type I18nValue } from "./i18n-context";
import { DEFAULT_LOCALE } from "./locales";
import { vi } from "./messages/vi/dictionary";
import { createTranslator } from "./translate";

export function I18nProvider({ children }: { children: ReactNode }) {
	const value = useMemo<I18nValue>(() => {
		const translator = createTranslator(DEFAULT_LOCALE, vi);
		// Set while rendering, so plain functions called during this render
		// (error messages) already speak the language on screen.
		setActiveTranslator(translator);
		return translator;
	}, []);

	useEffect(() => {
		document.documentElement.lang = value.locale;
	}, [value.locale]);

	return <I18nContext value={value}>{children}</I18nContext>;
}
