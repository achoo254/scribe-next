import type { Translate } from "../i18n/translate";

/**
 * The backend words its refusals (`{ error }`) in Vietnamese only. On a
 * page in another language, a refusal that carries a `code` (or a status
 * that always means the same thing) is worded here instead; any other
 * refusal falls back to the request's own translated message. Codes are
 * the backend's `AppError::Coded` codes.
 */
export function serverErrorText(
	t: Translate,
	status: number,
	code: string | null,
): string | null {
	switch (code) {
		case "NOT_TRANSCRIBED":
			return t("errors.server.notTranscribed");
		case "CONFIRM_REQUIRED":
			return t("errors.server.confirmRequired");
		case "RECORDING_OPEN":
			return t("account.delete.recordingOpen");
		case "CHAT_LINK_DISABLED":
			return t("errors.server.chatLinkDisabled");
		case "CHAT_NOT_LINKED":
			return t("errors.server.chatNotLinked");
		case "CHAT_BLOCKED":
			return t("errors.server.chatBlocked");
		case "NO_SUMMARY":
			return t("errors.server.noSummary");
		case "BOOKMARKS_FULL":
			return t("errors.server.bookmarksFull");
		case "NO_USER_KEY":
			return t("errors.server.noUserKey");
		case "USER_KEY_FAILED":
			return t("errors.server.userKeyFailed");
		case "USER_KEYS_DISABLED":
			return t("keys.dialog.unavailable");
		case "KEY_FORMAT":
			return t("errors.server.keyFormat");
		case "KEY_REJECTED":
			return t("errors.server.keyRejected");
		case "KEY_NO_CREDIT":
			return t("errors.server.keyNoCredit");
		case "KEY_CHECK_FAILED":
			return t("errors.server.keyCheckFailed");
		case "ACCOUNT_DELETED":
			return t("errors.server.accountDeleted");
	}
	switch (status) {
		case 401:
			return t("errors.server.signedOut");
		case 413:
			return t("errors.server.tooLarge");
		case 429:
			return t("errors.server.tooMany");
		case 503:
			return t("errors.server.unavailable");
		default:
			return null;
	}
}
