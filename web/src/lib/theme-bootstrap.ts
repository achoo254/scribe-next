import { themeBootstrapScript } from "@achoo254/app-shell/preferences";

/**
 * Scribe's localStorage key of the theme before the shared `hd_theme`
 * cookie: read while the visitor has no shared choice yet, so nobody loses
 * the theme they picked.
 */
export const LEGACY_THEME_KEY = "scribe-theme";

/**
 * The inline <head> script that sets `data-theme` before the first paint.
 * vite.config.ts puts it in index.html; its sha256 is allowed by the CSP in
 * deploy/nginx-scribe-security-headers.inc (scripts/theme-bootstrap.test.ts
 * keeps the two in step).
 */
export const THEME_BOOTSTRAP_SCRIPT = themeBootstrapScript({
	legacyThemeKey: LEGACY_THEME_KEY,
});
