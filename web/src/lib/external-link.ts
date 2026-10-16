/* Copied from localman-next apps/web/src/platform/navigation.ts
   (`safeExternalUrl`, `openExternal`). Scribe change: only these two; the
   in-app login redirects live in auth/auth-context.tsx. */

/**
 * The URL when it is an absolute http(s) address, else null. Links built
 * from data (mock URLs, IdP pages) must never run `javascript:` or open
 * `data:`/`file:` documents.
 */
export function safeExternalUrl(url: string): string | null {
	try {
		const { protocol } = new URL(url);
		return protocol === "http:" || protocol === "https:" ? url : null;
	} catch {
		return null;
	}
}

/**
 * Opens a page on another site in a new tab without sharing `opener`.
 * Anything but http(s) is ignored; returns whether a tab was opened.
 * In the iOS app Capacitor hands `window.open` to the system browser
 * (`WebViewDelegationHandler`, `createWebViewWith` → `UIApplication.open`),
 * so the same call works there.
 */
export function openExternal(url: string): boolean {
	if (safeExternalUrl(url) === null) return false;
	window.open(url, "_blank", "noopener,noreferrer");
	return true;
}
