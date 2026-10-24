/* Copied from localman-next apps/web/src/components/layout/ecosystem-app-switcher.tsx.
   Scribe changes: the tenant comes from `GET /api/idp` (lib/tenant-info.ts),
   the native shell is detected at run time (`isNativeShell`) and always
   lists the primary tenant's apps (the app talks to it only). */
import "@shared-next/app-switcher/app-switcher.css";
import { AppSwitcher } from "@shared-next/app-switcher";
import { useI18n } from "../i18n/i18n-context";
import { isNativeShell } from "../lib/api";
import { PRIMARY_TENANT_DOMAIN } from "../lib/brand";
import { openExternal } from "../lib/external-link";
import { useTenantInfo } from "../lib/tenant-info";

function openInSystemBrowser(url: string): void {
	openExternal(url);
}

/**
 * The 9-dot button listing the other apps the same account signs in to
 * (`@shared-next/app-switcher`). Signed-in top bar only.
 */
export function EcosystemAppSwitcher() {
	const { locale } = useI18n();
	const tenant = useTenantInfo();
	const native = isNativeShell();
	return (
		<AppSwitcher
			current="scribe"
			// The apps of this host's tenant; apps of the primary tenant only
			// (the switcher's `primaryTenantOnly`) are left out elsewhere.
			tenantDomain={native ? PRIMARY_TENANT_DOMAIN : tenant.domain}
			isPrimaryTenant={native || tenant.primary}
			locale={locale}
			// The shell's WebView must not load another app: system browser.
			onOpen={native ? openInSystemBrowser : undefined}
			className="app-header-switcher"
		/>
	);
}
