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

/** The word the user types to confirm, after normalizing ("XOÁ", "xóa", "xoa"). */
const CONFIRM_WORD = "xoa";

/** The typed confirmation matches, by the backend's own rule. */
export function confirmMatches(typed: string): boolean {
	return normalizeText(typed) === CONFIRM_WORD;
}

/**
 * Deletes the Scribe account of the session and everything in it; the
 * server also ends every session and device of the account. 409
 * `RECORDING_OPEN` while a browser recording is running. POST, not
 * `DELETE /api/me`: in the iOS app requests go through CapacitorHttp, whose
 * JSON body is proven on POST only.
 */
export async function deleteAccount(
	confirm: string,
	fetcher: typeof fetch = fetch,
): Promise<AccountDeletion> {
	const body = await apiJson<Partial<AccountDeletion>>(
		"/api/me/delete",
		{ method: "POST", json: { confirm } },
		fetcher,
	);
	return {
		deleted: body.deleted === true,
		files: typeof body.files === "number" ? body.files : 0,
		bytes: typeof body.bytes === "number" ? body.bytes : 0,
		complete: body.complete !== false,
	};
}
