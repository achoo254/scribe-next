import { ApiError, apiJson } from "../lib/api";

/**
 * Storage every account gets (Scribe has no plans: 200 MB, files up to
 * 200 MB, kept 7 days). The backend still sends `plan` with a constant
 * `name: "free"` and `until: null` for app bundles that read them; the web
 * reads only these limits.
 */
export interface StorageLimits {
	quotaBytes: number;
	maxFileBytes: number;
	/** Days a new file is kept before it is deleted. */
	retentionDays: number;
}

export interface CurrentUser {
	email: string;
	/** Display name from the IdP; null until the next sign-in, missing from older backends. */
	name?: string | null;
	/** IdP picture URL (https), like `name`; the menu shows the initial without it. */
	picture?: string | null;
	/**
	 * Always true for a signed-in user: every AI feature runs on the user's
	 * own key (`ownKeys`). Kept for app bundles that still read it.
	 */
	canTranscribe?: boolean;
	/** Storage limits (named `plan` for older app bundles); missing from older backends. */
	plan?: StorageLimits | null;
	/** Always null: Scribe meters no AI minutes. Kept for app bundles that still read it. */
	transcription?: null;
	/** Which uses run on the user's own keys ("Key riêng"); missing from older backends. */
	ownKeys?: OwnKeys | null;
}

/**
 * AI runs only on the user's own keys ("Key riêng"): the provider active
 * for each use (null: that use is unavailable until a valid key is added),
 * and the minutes transcribed this month on the user's keys. `available`
 * false: the server cannot store keys, so no AI use is available.
 */
export interface OwnKeys {
	available: boolean;
	transcribe: string | null;
	live: string | null;
	text: string | null;
	textModel: string | null;
	minutes: number;
}

/**
 * The signed-in user from the session cookie (httpOnly, so JS cannot read
 * it); null when there is no valid session or the backend is unreachable.
 */
export async function fetchCurrentUser(
	fetcher: typeof fetch = fetch,
): Promise<CurrentUser | null> {
	try {
		return await apiJson<CurrentUser>("/api/me", {}, fetcher);
	} catch (error) {
		if (error instanceof ApiError) return null;
		throw error;
	}
}

/**
 * Ends the backend session; returns where to end the hoandat-auth session
 * too (web), or null. A failure still signs the user out locally.
 */
export async function deleteSession(
	fetcher: typeof fetch = fetch,
): Promise<string | null> {
	const body = await apiJson<{ endSessionUrl?: string | null }>(
		"/api/auth/logout",
		{ method: "POST", json: {} },
		fetcher,
	).catch(() => null);
	return body?.endSessionUrl ?? null;
}

/** Where the web sign-in starts: the backend redirects to hoandat-auth. */
export const WEB_LOGIN_PATH = "/api/auth/login";

/**
 * Where a web sign-up starts: the same backend sign-in, but hoandat-auth
 * opens on "create account" (`prompt=create`, which links to sign-in for
 * existing accounts). Only sign-up buttons use it: the sign-up clicks of
 * each app are counted from `prompt=create` in the auth log.
 */
export const WEB_SIGN_UP_PATH = `${WEB_LOGIN_PATH}?signup=1`;
