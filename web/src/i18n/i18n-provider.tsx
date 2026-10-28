/* Adapted from localman-next apps/web/src/i18n/i18n-provider.tsx. The
   signed-in app speaks Vietnamese only, so the lazy dictionaries, the saved
   choice and the account sync of the source are left out: Vietnamese is
   imported statically (no chunk for an open tab to miss after a deploy).
   The signed-out pages switch language below this, in `PublicI18nProvider`;
   <html lang="vi"> comes from index.html. */
import { type ReactNode, useMemo } from "react";
import { setActiveTranslator } from "./active-translator";
import { I18nContext, type I18nValue, VIETNAMESE } from "./i18n-context";

export function I18nProvider({ children }: { children: ReactNode }) {
	const value = useMemo<I18nValue>(() => {
		// Set while rendering, so plain functions called during this render
		// (error messages) already speak the language on screen.
		setActiveTranslator(VIETNAMESE);
		return VIETNAMESE;
	}, []);

	return <I18nContext value={value}>{children}</I18nContext>;
}
