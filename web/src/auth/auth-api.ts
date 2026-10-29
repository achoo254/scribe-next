import { ApiError, apiJson } from "../lib/api";

/** Monthly transcription allowance of the signed-in user. */
export interface Transcription {
	plan: "free" | "plus";
	planUntil?: string | null;
	usedMinutes: number;
	limitMinutes: number;
	/** Start of next month (Vietnam time), when the minutes reset. */
	resetsAt: string;
	/** The shared monthly limit is used up: nobody on this plan can transcribe. */
	capReached: boolean;
	/** Transcription is paused for everyone (server key unusable). */
	paused?: boolean;
	/**
	 * Minutes left this month on the transcription hours the whole service
	 * shares (ElevenLabs); null when the provider has no such limit.
	 */
	sharedLeftMinutes?: number | null;
	/**
	 * An iPhone session is transcribed as a whole (one request, consistent
	 * speakers): send every segment and show the minutes of the session.
	 */
	wholeSession?: boolean;
	/** Free plan: minutes of today (Vietnam time); null on plus. */
	daily?: { usedMinutes: number; limitMinutes: number } | null;
}

/** The plan in effect and what it allows. */
export interface PlanInfo {
	name: "free" | "plus";
	/** End of a plus plan (ISO); null when it does not end. */
	until?: string | null;
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
	/** May transcribe recordings (backend allowlist). */
	canTranscribe?: boolean;
	/** Missing from older backends. */
	plan?: PlanInfo | null;
	transcription?: Transcription | null;
	/** Which uses run on the user's own keys ("Key riêng"); missing from older backends. */
	ownKeys?: OwnKeys | null;
}

/**
 * Uses that run on the user's own keys: the provider of each (null: on
 * Scribe's key, metered as usual) and the minutes transcribed this month
 * on the user's keys.
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
