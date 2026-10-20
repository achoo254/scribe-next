/* Mirrors localman-next apps/web/src/lib/tenant-info.ts. Scribe changes:
   only the account name (`GET /api/idp`; the IdP origin stays a build
   constant, lib/idp.ts), and the read starts once at boot (`main.tsx`)
   instead of on the first subscriber, so components only read the store. */
import { useSyncExternalStore } from "react";
import { apiJson } from "./api";

/**
 * Name of the account people sign in with, until the backend answers (and
 * if it cannot): the IdP's default tenant name, as the backend's own
 * fallback (`IDP_DISPLAY_NAME`).
 */
export const DEFAULT_IDP_NAME = "Auth Next";

/** Longest name shown; the backend applies the same limit. */
const MAX_NAME_LENGTH = 64;

/** The `displayName` of a `GET /api/idp` reply, or null when unusable. */
export function idpNameOf(body: unknown): string | null {
	if (typeof body !== "object" || body === null) return null;
	const value: unknown = Reflect.get(body, "displayName");
	if (typeof value !== "string") return null;
	const name = value.trim();
	return name && name.length <= MAX_NAME_LENGTH ? name : null;
}

let loaded: string | undefined;
let loading: Promise<void> | undefined;
const listeners = new Set<() => void>();

/**
 * Reads the name once per page load; a failure keeps the default. Called
 * from `main.tsx`; later calls return the same read.
 */
export function loadIdpName(fetcher: typeof fetch = fetch): Promise<void> {
	loading ??= apiJson<unknown>("/api/idp", {}, fetcher).then(
		(body) => {
			const name = idpNameOf(body);
			if (name === null) return;
			loaded = name;
			for (const notify of listeners) notify();
		},
		() => {
			// The default stays: every place still names the sign-in account.
		},
	);
	return loading;
}

function subscribe(listener: () => void): () => void {
	listeners.add(listener);
	return () => listeners.delete(listener);
}

/** The sign-in account's name: the default until the backend answers. */
export function useIdpName(): string {
	return useSyncExternalStore(
		subscribe,
		() => loaded ?? DEFAULT_IDP_NAME,
		() => DEFAULT_IDP_NAME,
	);
}
