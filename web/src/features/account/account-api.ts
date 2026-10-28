import { VIETNAMESE } from "../../i18n/i18n-context";
import { apiJson } from "../../lib/api";
import { normalizeText } from "../search/normalize";

/**
 * What `POST /api/me/delete` answers: the Scribe account and its data are gone.
 * `complete: false` when part of the data could not be removed yet (the
 * server finishes it within the hour).
 */
export interface AccountDeletion {
	deleted: boolean;
	files: number;
	bytes: number;
	complete: boolean;
}

/**
 * The confirmation the backend checks (`confirm_matches`): the Vietnamese
 * word ("XOÁ"), whatever the language on screen.
 */
const BACKEND_CONFIRM_WORD = VIETNAMESE.t("account.delete.confirmWord");

/**
 * The typed confirmation matches the word the dialog asks for in the
 * language on screen ("XOÁ", "DELETE"), ignoring case and accents.
 */
export function confirmMatches(typed: string, word: string): boolean {
	return normalizeText(typed) === normalizeText(word);
}

/**
 * Deletes the Scribe account of the session and everything in it; the
 * server also ends every session and device of the account. The dialog
 * has the user type its word (`confirmMatches`); the request carries the
 * backend's own word, so English users type "DELETE". 409
 * `RECORDING_OPEN` while a browser recording is running. POST, not
 * `DELETE /api/me`: in the iOS app requests go through CapacitorHttp, whose
 * JSON body is proven on POST only.
 */
export async function deleteAccount(
	fetcher: typeof fetch = fetch,
): Promise<AccountDeletion> {
	const body = await apiJson<Partial<AccountDeletion>>(
		"/api/me/delete",
		{ method: "POST", json: { confirm: BACKEND_CONFIRM_WORD } },
		fetcher,
	);
	return {
		deleted: body.deleted === true,
		files: typeof body.files === "number" ? body.files : 0,
		bytes: typeof body.bytes === "number" ? body.bytes : 0,
		complete: body.complete !== false,
	};
}
