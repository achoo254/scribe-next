/* Mirrors localman-next apps/web/src/lib/tenant-info.ts. Scribe changes:
   the reply is `GET /api/idp` (also the sign-in account's name, so the app
   reads it once, at boot in `main.tsx`, instead of on the first
   subscriber); a host Scribe does not serve, or a tenant whose owner has
   not turned Scribe on, is "disabled" (the app shows the "not enabled"
   page and never calls a signed-in API); a tenant's host whose
   `/api/idp` fails is "unavailable" (a retry page, never the primary
   tenant's names and links); the native shell talks to the primary tenant
   only and keeps the build's IdP origin. */
import { useSyncExternalStore } from "react";
import { ApiError, apiJson, isNativeShell, onTenantDisabled } from "./api";
import { PRIMARY_TENANT_DOMAIN } from "./brand";
import { DEFAULT_IDP_ORIGIN, idpOriginOf } from "./idp";

/**
 * Scribe on this domain: on, not (yet) turned on by the domain's owner, or
 * not known because `/api/idp` failed on a tenant's host.
 */
export type ScribeState = "enabled" | "disabled" | "unavailable";

/** `/api/idp` answers within this long; then the app stops waiting. */
const TENANT_TIMEOUT_MS = 10_000;

/** The tenant of the host the page is served on (`GET /api/idp`). */
export interface TenantInfo {
	/** Name of the account people sign in with ("Auth Next", "Acme"). */
	readonly displayName: string;
	/** The tenant's domain (`hoandat.com`, `acme.vn`). */
	readonly domain: string;
	/** Origin of the tenant's IdP (`https://auth.acme.vn`). */
	readonly issuer: string;
	/** The primary tenant: the only one with the mobile apps, chat and plans. */
	readonly primary: boolean;
	readonly scribe: ScribeState;
}

/**
 * Name of the account people sign in with, until the backend answers (and
 * if it cannot): the IdP's default tenant name, as the backend's own
 * fallback (`IDP_DISPLAY_NAME`).
 */
export const DEFAULT_IDP_NAME = "Auth Next";

/** Longest name shown; the backend applies the same limit. */
const MAX_NAME_LENGTH = 64;

/** What the app shows until `/api/idp` answers, and if it cannot: the primary tenant. */
export const DEFAULT_TENANT_INFO: TenantInfo = {
	displayName: DEFAULT_IDP_NAME,
	domain: PRIMARY_TENANT_DOMAIN,
	issuer: DEFAULT_IDP_ORIGIN,
	primary: true,
	scribe: "enabled",
};

function field(body: object, name: string): unknown {
	return Reflect.get(body, name);
}

/** The `displayName` of a `GET /api/idp` reply, or null when unusable. */
export function idpNameOf(body: unknown): string | null {
	if (typeof body !== "object" || body === null) return null;
	const value = field(body, "displayName");
	if (typeof value !== "string") return null;
	const name = value.trim();
	return name && name.length <= MAX_NAME_LENGTH ? name : null;
}

/** A domain name as the backend sends it, or null. */
function domainOf(value: unknown): string | null {
	if (typeof value !== "string") return null;
	const domain = value.trim().toLowerCase();
	return /^[a-z0-9.-]+\.[a-z0-9-]+$/.test(domain) ? domain : null;
}

/**
 * The tenant of a `GET /api/idp` reply. Another tenant needs a valid
 * domain and issuer; any missing or mistyped field otherwise falls back to
 * the primary tenant (an older backend answers `{ displayName }` alone).
 */
export function tenantInfoOf(body: unknown): TenantInfo {
	const displayName = idpNameOf(body) ?? DEFAULT_IDP_NAME;
	if (typeof body !== "object" || body === null) return DEFAULT_TENANT_INFO;
	const domain = domainOf(field(body, "domain"));
	const issuerValue = field(body, "issuer");
	const issuer =
		typeof issuerValue === "string" ? idpOriginOf(issuerValue) : null;
	if (field(body, "primary") === false && domain && issuer) {
		const scribe: ScribeState =
			field(body, "scribe") === "enabled" ? "enabled" : "disabled";
		return { displayName, domain, issuer, primary: false, scribe };
	}
	return { ...DEFAULT_TENANT_INFO, displayName };
}

/**
 * A host Scribe does not serve (404 `UNKNOWN_TENANT`): "not enabled" on the
 * domain of the address bar (`scribe.<domain>`).
 */
export function unknownTenantInfo(hostname: string): TenantInfo {
	const host = hostname.toLowerCase();
	const domain = host.startsWith("scribe.")
		? host.slice("scribe.".length)
		: host;
	return {
		displayName: DEFAULT_IDP_NAME,
		domain,
		issuer: `https://auth.${domain}`,
		primary: false,
		scribe: "disabled",
	};
}

/**
 * True for a host of another tenant (`scribe.<domain>` other than the
 * primary one): there a failed `/api/idp` must not show the primary tenant.
 */
export function isTenantHost(hostname: string): boolean {
	const host = hostname.toLowerCase();
	return (
		host.startsWith("scribe.") && host !== `scribe.${PRIMARY_TENANT_DOMAIN}`
	);
}

let loaded: TenantInfo | undefined;
/** True once `/api/idp` answered or failed: the app stops waiting. */
let settled = false;
let loading: Promise<void> | undefined;
const listeners = new Set<() => void>();

function publish(info: TenantInfo | undefined): void {
	if (info) loaded = info;
	settled = true;
	for (const notify of listeners) notify();
}

/**
 * Reads the tenant once per page load; a failure keeps the defaults. Called
 * from `main.tsx`; later calls return the same read. In the native shell
 * only the name is taken: the app talks to the primary tenant only.
 */
export function loadTenantInfo(fetcher: typeof fetch = fetch): Promise<void> {
	loading ??= apiJson<unknown>(
		"/api/idp",
		{ signal: AbortSignal.timeout(TENANT_TIMEOUT_MS) },
		fetcher,
	).then(
		(body) => {
			const info = tenantInfoOf(body);
			publish(
				isNativeShell()
					? { ...DEFAULT_TENANT_INFO, displayName: info.displayName }
					: info,
			);
		},
		(error: unknown) => {
			if (isNativeShell()) {
				publish(undefined);
				return;
			}
			const hostname = globalThis.location.hostname;
			if (error instanceof ApiError && error.code === "UNKNOWN_TENANT") {
				publish(unknownTenantInfo(hostname));
			} else if (isTenantHost(hostname)) {
				publish({ ...unknownTenantInfo(hostname), scribe: "unavailable" });
			} else {
				publish(undefined);
			}
		},
	);
	return loading;
}

/** A signed-in API said the tenant was turned off meanwhile: show the "not enabled" page. */
onTenantDisabled(() => {
	const current = loaded ?? DEFAULT_TENANT_INFO;
	if (current.primary || current.scribe === "disabled") return;
	publish({ ...current, scribe: "disabled" });
});

function subscribe(listener: () => void): () => void {
	listeners.add(listener);
	return () => listeners.delete(listener);
}

/** The tenant of this host: the primary tenant's defaults until `/api/idp` answers. */
export function useTenantInfo(): TenantInfo {
	return useSyncExternalStore(
		subscribe,
		() => loaded ?? DEFAULT_TENANT_INFO,
		() => DEFAULT_TENANT_INFO,
	);
}

/** False while `/api/idp` has not answered yet (nor failed). */
export function useTenantSettled(): boolean {
	return useSyncExternalStore(
		subscribe,
		() => settled,
		() => false,
	);
}
