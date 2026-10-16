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
}

export interface CurrentUser {
	email: string;
	/** May transcribe recordings (backend allowlist). */
	canTranscribe?: boolean;
	transcription?: Transcription | null;
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
