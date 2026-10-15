import { ApiError, apiFetch, apiJson } from "../lib/api";

export interface CurrentUser {
	email: string;
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

/** Trades a Firebase ID token for the backend session cookie. */
export function createSession(
	idToken: string,
	fetcher: typeof fetch = fetch,
): Promise<CurrentUser> {
	return apiJson<CurrentUser>(
		"/api/auth/session",
		{ method: "POST", json: { idToken }, fallbackError: "Đăng nhập thất bại" },
		fetcher,
	);
}

/** Ends the backend session; a failure still signs the user out locally. */
export async function deleteSession(
	fetcher: typeof fetch = fetch,
): Promise<void> {
	await apiFetch("/api/auth/logout", { method: "POST" }, fetcher).catch(
		() => undefined,
	);
}
