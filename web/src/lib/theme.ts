export type ThemePreference = "light" | "dark" | "system";

/** Must match the inline bootstrap script in index.html. */
export const THEME_STORAGE_KEY = "onescribe-theme";

export function isThemePreference(value: unknown): value is ThemePreference {
	return value === "light" || value === "dark" || value === "system";
}

/** Reads the saved preference; storage may be unavailable (privacy mode). */
export function readThemePreference(): ThemePreference {
	try {
		const stored = window.localStorage.getItem(THEME_STORAGE_KEY);
		return isThemePreference(stored) ? stored : "system";
	} catch {
		return "system";
	}
}

const listeners = new Set<() => void>();

/** Notifies `listener` when the preference changes (every theme switch). */
export function subscribeThemePreference(listener: () => void): () => void {
	listeners.add(listener);
	return () => {
		listeners.delete(listener);
	};
}

/**
 * Applies the preference to <html data-theme>. "system" removes the
 * attribute so tokens.css follows `prefers-color-scheme`.
 */
export function applyThemePreference(
	preference: ThemePreference,
	root: HTMLElement = document.documentElement,
): void {
	if (preference === "system") {
		delete root.dataset.theme;
	} else {
		root.dataset.theme = preference;
	}
	try {
		window.localStorage.setItem(THEME_STORAGE_KEY, preference);
	} catch {
		// Storage unavailable: the choice still applies for this page view.
	}
	for (const listener of listeners) listener();
}

/**
 * Copies the resolved `--c-bg` token into <meta name="theme-color"> so the
 * browser chrome (iOS status bar, Android toolbar) matches the page. Runs
 * at startup, on every theme switch and when the OS scheme changes.
 */
export function syncThemeColor(
	root: HTMLElement = document.documentElement,
): void {
	const meta = document.querySelector('meta[name="theme-color"]');
	const background = getComputedStyle(root).getPropertyValue("--c-bg").trim();
	if (meta && background) meta.setAttribute("content", background);
}

/** Keeps theme-color in sync from now on; returns the cleanup. */
export function watchThemeColor(): () => void {
	syncThemeColor();
	const media = window.matchMedia("(prefers-color-scheme: dark)");
	const onChange = (): void => syncThemeColor();
	media.addEventListener("change", onChange);
	const unsubscribe = subscribeThemePreference(onChange);
	return () => {
		media.removeEventListener("change", onChange);
		unsubscribe();
	};
}
