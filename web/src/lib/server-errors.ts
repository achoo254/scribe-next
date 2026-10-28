import type { Translate } from "../i18n/translate";
import type { QuotaRefusal } from "./api";
import { formatTimeAndDay } from "./format";

/**
 * The backend words its refusals (`{ error }`) in Vietnamese only. On a
 * page in another language, a refusal that carries a `code` (or a status
 * that always means the same thing) is worded here instead; any other
 * refusal falls back to the request's own translated message. Codes are
 * the backend's `AppError::Coded` and `AppError::Quota` codes.
 */
export function serverErrorText(
	t: Translate,
	status: number,
	code: string | null,
	quota: QuotaRefusal | null,
): string | null {
	const used = quota?.used ?? 0;
	const limit = quota?.limit ?? 0;
	const time = formatTimeAndDay(quota?.retryAt);
	switch (code) {
		case "QUOTA_EXCEEDED":
			return t("errors.server.quotaExceeded", { used, limit });
		case "DAILY_LIMIT":
			return t("errors.server.dailyLimit", { used, limit, time });
		case "TOO_LONG_FOR_FREE":
			return t("errors.server.tooLongForFree", { used, limit });
		case "ACCOUNT_TOO_NEW":
			return t("errors.server.accountTooNew", { time });
		case "PROVIDER_HOURS":
			return t("errors.server.providerHours", { time });
		case "CAP_REACHED":
			// Summaries, questions and translations word their own (with the time).
			return t("transcript.quota.capReached");
		case "SUMMARY_DAILY_LIMIT":
			return t("transcript.minutes.dailyLimit", { used, limit, time });
		case "SUMMARY_MONTHLY_LIMIT":
			return t("transcript.minutes.monthlyLimit", { used, limit, time });
		case "ASK_DAILY_LIMIT":
			return t("search.ask.dailyLimit", { used, limit, time });
		case "ASK_MONTHLY_LIMIT":
			return t("search.ask.monthlyLimit", { used, limit, time });
		case "TRANSLATE_DAILY_LIMIT":
			return t("transcript.translate.dailyLimit", { used, limit, time });
		case "TRANSLATE_MONTHLY_LIMIT":
			return t("transcript.translate.monthlyLimit", { used, limit, time });
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
		case "REALTIME_NOT_ALLOWED":
			return t("errors.server.realtimeNotAllowed");
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
