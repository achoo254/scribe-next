/* Copied from localman-next apps/web/src/components/layout/ecosystem-app-switcher.tsx.
   Scribe changes: no tenant lookup (Scribe runs on the primary tenant
   only) and the native shell is detected at run time (`isNativeShell`). */
import "@achoo254/app-switcher/app-switcher.css";
import { AppSwitcher } from "@achoo254/app-switcher";
import { useI18n } from "../i18n/i18n-context";
import { isNativeShell } from "../lib/api";
import { PRIMARY_TENANT_DOMAIN } from "../lib/brand";
import { openExternal } from "../lib/external-link";

function openInSystemBrowser(url: string): void {
	openExternal(url);
}

/**
 * The 9-dot button listing the other apps the same account signs in to
 * (`@achoo254/app-switcher`). Signed-in top bar only.
 */
export function EcosystemAppSwitcher() {
	const { locale } = useI18n();
	return (
		<AppSwitcher
			current="scribe"
			domain={PRIMARY_TENANT_DOMAIN}
			locale={locale}
			// The shell's WebView must not load another app: system browser.
			onOpen={isNativeShell() ? openInSystemBrowser : undefined}
			className="app-header-switcher"
		/>
	);
}
