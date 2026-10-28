import type { SourceMessages, Translation } from "../../message-types";

/** English of `vi/recorder.ts`. Settings paths follow iOS in English. */
export const recorder: Translation<SourceMessages["recorder"]> = {
	defaultName: "Recording {date}",
	micHint: {
		iosEdge:
			"Go to iOS Settings → Edge → Microphone → on, then reload the page and try again.",
		iosChrome:
			"Go to iOS Settings → Chrome → Microphone → on, then reload the page and try again.",
		iosFirefox:
			"Go to iOS Settings → Firefox → Microphone → on, then reload the page and try again.",
		iosSafari:
			"Tap “aA” left of the address bar → Website Settings → Microphone → Allow (or iOS Settings → Safari → Microphone), then reload the page.",
		desktop:
			"Click the lock/permissions icon left of the address bar → allow Microphone, then reload the page and try again.",
	},
	startError: {
		screenDenied:
			"Screen sharing / microphone permission not granted. Choose what to share and allow access, then try again.",
		micDenied: "The browser has no Microphone permission. {hint}",
		notFound: "No suitable microphone found.",
		busy: "Another app is using the microphone. Close it and try again.",
		aborted:
			"Couldn't access the microphone (device busy or interrupted). Try again.",
		insecure: "Recording needs a secure connection (HTTPS).",
		failed: "Couldn't start recording.",
		denied: "Couldn't start recording (access denied?)",
	},
	pauseError: "Couldn't pause or resume.",
	interrupt: {
		micEnded:
			"The recording stopped (microphone lost, maybe because the screen locked). What was recorded is saved.",
		hidden:
			"The recording stopped when the screen locked or you switched apps. What was recorded is saved.",
	},
	storageFull:
		"{message} The recording stopped and was saved up to that point.",
	chunksLost:
		"Some parts hit network errors. What uploaded is saved (the recording may be cut short).",
	noSource:
		"Couldn't get any source. Check microphone / screen sharing permissions.",
	mediaError: "Recording error (MediaRecorder), stopped",
	invalidSession: "Invalid recording session",
	finalizeFailed: "Couldn't finish the recording",
	chunkFailed: "Chunk upload failed",
	uploadFailed: "Recording upload failed",
	native: {
		signInTitle: "Your session has expired",
		signInBody: "Sign in again to upload the parts waiting on this device.",
		quotaTitle: "Out of storage",
		quotaBody:
			"{message} The parts stay on this device and upload when there is room.",
		lostTitle: "Lost the end of a recording",
		lostBody:
			"The app closed while recording: up to {seconds} seconds of the open part were lost.",
		syncTitle: "Couldn't sync the parts recorded on this device",
		syncRetry: "Reopen the app to try again.",
	},
};
