/*
 * The theme is the shared choice of every hoandat.com app
 * (@achoo254/app-shell: the `hd_theme` cookie on the root domain, this
 * WebView's storage in the native shells). The footer switches, signed in
 * or not, write it; this module keeps the page and the browser chrome on it.
 */
import {
	type PreferenceOptions,
	type ThemePreference,
	applyTheme,
	readPreferences,
	subscribePreferences,
} from "@achoo254/app-shell/preferences";
import { PRIMARY_TENANT_DOMAIN } from "./brand";
import { LEGACY_THEME_KEY } from "./theme-bootstrap";

export type { ThemePreference };

/** Where Scribe reads and writes the shared choices. */
export const PREFERENCE_OPTIONS: PreferenceOptions = {
	domain: PRIMARY_TENANT_DOMAIN,
	legacyThemeKey: LEGACY_THEME_KEY,
};

/**
 * Copies the resolved `--c-bg` token into <meta name="theme-color"> so the
 * browser chrome (iOS status bar, Android toolbar) matches the page.
 */
export function syncThemeColor(
	root: HTMLElement = document.documentElement,
): void {
	const meta = document.querySelector('meta[name="theme-color"]');
	const background = getComputedStyle(root).getPropertyValue("--c-bg").trim();
	if (meta && background) meta.setAttribute("content", background);
}

/**
 * From now on, applies the stored theme whenever it changes (a switch on
 * this page, or another app's choice noticed when the page comes back into
 * view) and keeps theme-color in sync, also when the OS scheme changes.
 * Returns the cleanup.
 */
export function watchTheme(): () => void {
	syncThemeColor();
	const media = window.matchMedia("(prefers-color-scheme: dark)");
	const onSchemeChange = (): void => syncThemeColor();
	media.addEventListener("change", onSchemeChange);
	const unsubscribe = subscribePreferences(() => {
		applyTheme(readPreferences(PREFERENCE_OPTIONS).theme);
		syncThemeColor();
	});
	return () => {
		media.removeEventListener("change", onSchemeChange);
		unsubscribe();
	};
}
