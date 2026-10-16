import { activeTranslator } from "../i18n/active-translator";
import type { MessageKey } from "../i18n/message-types";
import { errorMessage } from "../lib/api";

/**
 * Codes the backend appends to /login?error=… when a web sign-in through
 * hoandat-auth does not complete (`auth.rs`).
 */
const LOGIN_ERRORS = {
	denied: "login.errors.denied",
	expired: "login.errors.expired",
	invalid: "login.errors.invalid",
	unverified: "login.errors.unverified",
	forbidden: "login.errors.forbidden",
	idp: "login.errors.idp",
	busy: "login.errors.busy",
	failed: "login.errors.failed",
} as const satisfies Readonly<Record<string, MessageKey>>;

function isLoginErrorCode(code: string): code is keyof typeof LOGIN_ERRORS {
	return Object.hasOwn(LOGIN_ERRORS, code);
}

/** Text for an `?error=` code of /login; empty when there is none. */
export function loginErrorFromCode(code: string | null): string {
	if (!code) return "";
	return activeTranslator().t(
		isLoginErrorCode(code) ? LOGIN_ERRORS[code] : LOGIN_ERRORS.failed,
	);
}

/** Text for an error thrown by the iOS sign-in sheet or the backend (already Vietnamese). */
export function loginErrorMessage(error: unknown): string {
	return errorMessage(error, activeTranslator().t("login.failed"));
}
