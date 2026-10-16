/** Messages for recording failures, with where to fix them. */
import { activeTranslator } from "../../i18n/active-translator";

/** Recording sources, each switched on or off by the user (≥ 1 on). */
export interface RecordSources {
	mic: boolean;
	systemAudio: boolean;
	screen: boolean;
}

export interface BrowserInfo {
	userAgent: string;
	maxTouchPoints: number;
}

function currentBrowser(): BrowserInfo {
	if (typeof navigator === "undefined") {
		return { userAgent: "", maxTouchPoints: 0 };
	}
	return {
		userAgent: navigator.userAgent,
		maxTouchPoints: navigator.maxTouchPoints ?? 0,
	};
}

/**
 * Where to allow the microphone, by browser. A web page cannot open the
 * system settings, only point to them: Edge/Chrome/Firefox on iOS keep the
 * permission in iOS Settings, Safari in "aA" → website settings, desktop
 * and Android in the permission icon of the address bar.
 */
export function micPermissionHint(
	browser: BrowserInfo = currentBrowser(),
): string {
	const { t } = activeTranslator();
	const ua = browser.userAgent;
	const isIOS =
		/iPad|iPhone|iPod/.test(ua) ||
		(ua.includes("Macintosh") && browser.maxTouchPoints > 1);
	if (isIOS) {
		if (ua.includes("EdgiOS")) return t("recorder.micHint.iosEdge");
		if (ua.includes("CriOS")) return t("recorder.micHint.iosChrome");
		if (ua.includes("FxiOS")) return t("recorder.micHint.iosFirefox");
		return t("recorder.micHint.iosSafari");
	}
	return t("recorder.micHint.desktop");
}

/**
 * Message for a failed start. getUserMedia/getDisplayMedia reject with a
 * named DOMException; NotAllowedError (permission denied or picker
 * cancelled) is the common case on iOS WebKit. Other errors (backend,
 * network) keep their own message.
 */
export function startErrorMessage(
	error: unknown,
	sources: RecordSources,
	browser: BrowserInfo = currentBrowser(),
): string {
	const { t } = activeTranslator();
	if (error instanceof DOMException) {
		switch (error.name) {
			case "NotAllowedError":
				return sources.screen || sources.systemAudio
					? t("recorder.startError.screenDenied")
					: t("recorder.startError.micDenied", {
							hint: micPermissionHint(browser),
						});
			case "NotFoundError":
			case "OverconstrainedError":
				return t("recorder.startError.notFound");
			case "NotReadableError":
				return t("recorder.startError.busy");
			case "AbortError":
				return t("recorder.startError.aborted");
			case "SecurityError":
				return t("recorder.startError.insecure");
			default:
				return error.message || t("recorder.startError.failed");
		}
	}
	return error instanceof Error && error.message
		? error.message
		: t("recorder.startError.denied");
}
