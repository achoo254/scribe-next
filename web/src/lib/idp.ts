import { PRIMARY_ISSUER } from "./brand";
import type { TenantInfo } from "./tenant-info";

/**
 * hoandat-auth, the identity provider: its account pages are linked from
 * the account menu and the footer. On the primary tenant (and in the
 * native shell, which talks to it only) the origin is a build constant
 * (`VITE_IDP_ORIGIN`, see web/.env.example) rather than read from the
 * backend; on another tenant it is that tenant's issuer (`GET /api/idp`,
 * lib/tenant-info.ts). The default is the primary tenant's issuer in
 * brand.config.json.
 */
export const DEFAULT_IDP_ORIGIN = PRIMARY_ISSUER;

/**
 * Origin of an http(s) URL (`https://auth.example.com`), or null when the
 * value is not one. vite.config.ts rejects such a value at build time.
 */
export function idpOriginOf(value: string): string | null {
	try {
		const url = new URL(value);
		return url.protocol === "https:" || url.protocol === "http:"
			? url.origin
			: null;
	} catch {
		return null;
	}
}

const IDP_ORIGIN =
	idpOriginOf(import.meta.env.VITE_IDP_ORIGIN ?? "") ?? DEFAULT_IDP_ORIGIN;

/** The primary tenant's account page (build origin). */
export const IDP_ACCOUNT_URL = `${IDP_ORIGIN}/account`;

/** Apps the user has authorised, a section of the account page. */
export const IDP_APPS_URL = `${IDP_ACCOUNT_URL}#account-apps`;

/** Origin of the tenant's IdP: the build origin on the primary tenant. */
export function idpOriginFor(tenant: TenantInfo): string {
	return tenant.primary ? IDP_ORIGIN : tenant.issuer;
}

/** The user's account page at the tenant's IdP. */
export function idpAccountUrlOf(tenant: TenantInfo): string {
	return `${idpOriginFor(tenant)}/account`;
}

/** Apps the user has authorised at the tenant's IdP. */
export function idpAppsUrlOf(tenant: TenantInfo): string {
	return `${idpAccountUrlOf(tenant)}#account-apps`;
}

/** Where the domain's owner turns Scribe on (AI keys, `/admin` of the tenant's IdP). */
export function idpAdminUrlOf(tenant: TenantInfo): string {
	return `${idpOriginFor(tenant)}/admin`;
}
