import type { CurrentUser } from "../../auth/auth-api";
import { errorCode } from "../../lib/api";
import type { KeyUse } from "./key-uses";

/**
 * Provider of the user's own key active for `use`, or null: Scribe runs AI
 * only on the user's keys, so null means that use is unavailable until a
 * valid key is added in "Key riêng" (or the server cannot store keys).
 */
export function ownKeyFor(
	user: CurrentUser | null | undefined,
	use: KeyUse,
): string | null {
	const own = user?.ownKeys;
	if (!own?.available) return null;
	return own[use];
}

/**
 * ElevenLabs transcribes a whole iPhone session in one request (consistent
 * speakers): every segment of the session is sent. OpenAI and Gemini
 * transcribe file by file.
 */
export function transcribesWholeSession(
	user: CurrentUser | null | undefined,
): boolean {
	return ownKeyFor(user, "transcribe") === "elevenlabs";
}

/**
 * The backend refused an AI request for a reason "Key riêng" fixes: no
 * valid own key for that use (`NO_USER_KEY`), or the provider rejected the
 * key (`USER_KEY_FAILED`).
 */
export function isOwnKeyRefusal(error: unknown): boolean {
	const code = errorCode(error);
	return code === "NO_USER_KEY" || code === "USER_KEY_FAILED";
}
