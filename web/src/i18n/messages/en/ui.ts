import type { SourceMessages, Translation } from "../../message-types";

/** English of `vi/ui.ts`. */
export const ui: Translation<SourceMessages["ui"]> = {
	loading: "Loading…",
	usage: {
		region: "Usage",
		retention: {
			one: "Files deleted after {count} day",
			other: "Files deleted after {count} days",
		},
		storage: "Storage",
		storageMeter: "Storage used",
		ownKey: {
			transcribe: "Transcription",
			text: "Minutes, questions, translation",
			value: "Using your key ({provider})",
			minutes: {
				one: "{count} minute on your key this month",
				other: "{count} minutes on your key this month",
			},
		},
		storageFull:
			"Storage is full: delete some recordings or files to keep recording and uploading.",
	},
	player: {
		playFailed: "Couldn't play: {error}",
		loadingRecording: "Loading the recording to play…",
		loadingAudio: "Loading audio…",
		play: "Play",
		pause: "Pause",
		seek: "Seek",
		mute: "Mute",
		unmute: "Unmute",
		volume: "Volume",
	},
	toast: {
		region: "Notifications",
		close: "Close notification",
	},
};
