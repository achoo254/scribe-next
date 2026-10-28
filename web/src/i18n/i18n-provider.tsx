/* Adapted from localman-next apps/web/src/i18n/i18n-provider.tsx. Scribe
   changes: the language is the shared hoandat.com choice (`hd_lang`, set by
   the footer language switch of any app, signed in or not) instead of an
   account setting, and both dictionaries are imported statically (no chunk
   for an open tab to miss after a deploy). */
import {
	matchShellLocale,
	readPreferences,
	subscribePreferences,
} from "@achoo254/app-shell";
import { type ReactNode, useLayoutEffect, useSyncExternalStore } from "react";
import { PREFERENCE_OPTIONS } from "../lib/theme";
import { activeTranslator, setActiveTranslator } from "./active-translator";
import { I18nContext, type I18nValue, VIETNAMESE } from "./i18n-context";
import { DEFAULT_LOCALE, type Locale } from "./locales";
import type { Messages } from "./message-types";
import { en } from "./messages/en/dictionary";
import { createTranslator } from "./translate";

/**
 * The language of the page: the shared choice (`hd_lang` of any
 * hoandat.com app), else the device's language when Scribe speaks it, else
 * Vietnamese.
 */
export function readPreferredLocale(): Locale {
	return (
		readPreferences(PREFERENCE_OPTIONS).lang ??
		matchShellLocale(globalThis.navigator?.language) ??
		DEFAULT_LOCALE
	);
}

const serverLocale = (): Locale => DEFAULT_LOCALE;

/** The preferred locale, re-read after every choice (here or in another app). */
export function usePreferredLocale(): Locale {
	return useSyncExternalStore(
		subscribePreferences,
		readPreferredLocale,
		serverLocale,
	);
}

/**
 * `onLocaleChange` of the shared language switches: the switch stores the
 * choice itself (`hd_lang`) and `I18nProvider` re-renders the app in it,
 * so nothing is left to do.
 */
export function followStoredLocale(): void {}

/** One translator per language for the life of the page. */
const TRANSLATORS: Readonly<Record<Locale, I18nValue>> = {
	vi: VIETNAMESE,
	en: createTranslator("en", en satisfies Messages),
};

/**
 * Speaks the visitor's language to the whole app, signed in or not: the
 * texts, `<html lang>`, and the translator that plain functions use (error
 * messages, dates). A pick in the footer switch re-renders the app in the
 * new language without a reload, keeping a recording in progress.
 */
export function I18nProvider({ children }: { children: ReactNode }) {
	const locale = usePreferredLocale();
	const value = TRANSLATORS[locale];
	// Set while rendering (idempotent), so plain functions called during
	// this render already speak the language on screen.
	setActiveTranslator(value);

	useLayoutEffect(() => {
		setActiveTranslator(value);
		document.documentElement.lang = value.locale;
		return () => {
			// Unmounted (tests): plain functions go back to the default.
			if (activeTranslator() === value) setActiveTranslator(VIETNAMESE);
		};
	}, [value]);

	return <I18nContext value={value}>{children}</I18nContext>;
}
