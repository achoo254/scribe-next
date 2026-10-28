import {
	matchShellLocale,
	readPreferences,
	subscribePreferences,
} from "@achoo254/app-shell";
import {
	type ReactNode,
	useLayoutEffect,
	useMemo,
	useSyncExternalStore,
} from "react";
import { PREFERENCE_OPTIONS } from "../lib/theme";
import { activeTranslator, setActiveTranslator } from "./active-translator";
import { I18nContext, type I18nValue, VIETNAMESE } from "./i18n-context";
import { DEFAULT_LOCALE, type Locale } from "./locales";
import type { Messages } from "./message-types";
import { en } from "./messages/en/dictionary";
import { vi } from "./messages/vi/dictionary";
import { createTranslator } from "./translate";

const DICTIONARIES: Readonly<Record<Locale, Messages>> = { vi, en };

/**
 * Language of the signed-out pages: the visitor's shared choice (`hd_lang`,
 * set by the footer switch of any hoandat.com app), else the device's
 * language when Scribe speaks it, else Vietnamese.
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
 * Speaks the visitor's language to the signed-out pages below it (landing,
 * sign-in, the legal pages' layout) while they are on screen: their texts,
 * `<html lang>` and the translator plain functions use (sign-in errors).
 * Leaving them (signing in) gives the page back to Vietnamese.
 */
export function PublicI18nProvider({ children }: { children: ReactNode }) {
	const locale = usePreferredLocale();
	const value = useMemo<I18nValue>(() => {
		const translator =
			locale === DEFAULT_LOCALE
				? VIETNAMESE
				: createTranslator(locale, DICTIONARIES[locale]);
		// Set while rendering, so plain functions called during this render
		// (sign-in errors) already speak the language on screen.
		setActiveTranslator(translator);
		return translator;
	}, [locale]);

	useLayoutEffect(() => {
		const root = document.documentElement;
		const previousLang = root.lang;
		setActiveTranslator(value);
		root.lang = value.locale;
		return () => {
			root.lang = previousLang || DEFAULT_LOCALE;
			if (activeTranslator() === value) setActiveTranslator(VIETNAMESE);
		};
	}, [value]);

	return <I18nContext value={value}>{children}</I18nContext>;
}
