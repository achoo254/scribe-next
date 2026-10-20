import { PRIMARY_ISSUER } from "./brand";

/**
 * hoandat-auth, the identity provider: its account pages are linked from
 * the account menu and the footer. The origin is a build constant
 * (`VITE_IDP_ORIGIN`, see web/.env.example) rather than read from the
 * backend: `/api/auth/native/config` has no issuer and runs OIDC discovery
 * on every call. The default is the primary tenant's issuer in
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

/** The user's hoandat account page. */
export const IDP_ACCOUNT_URL = `${IDP_ORIGIN}/account`;

/** Apps the user has authorised, a section of the account page. */
export const IDP_APPS_URL = `${IDP_ACCOUNT_URL}#account-apps`;
