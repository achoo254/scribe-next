import type { SourceMessages, Translation } from "../../message-types";

/** English of `vi/ui.ts`. */
export const ui: Translation<SourceMessages["ui"]> = {
	loading: "Loading…",
	usage: {
		region: "Plan and usage",
		plan: { free: "Free plan", plus: "Plus plan" },
		planUntil: "until {date}",
		retention: {
			one: "Files deleted after {count} day",
			other: "Files deleted after {count} days",
		},
		resets: "minutes reset on {date}",
		storage: "Storage",
		storageMeter: "Storage used",
		month: "Transcribed this month",
		monthMeter: "Transcription minutes used this month",
		today: "Today",
		todayMeter: "Transcription minutes used today",
		minutes: "{used}/{limit} min",
		ownKey: {
			transcribe: "Transcription",
			text: "Minutes, questions, translation",
			value: "Using your key ({provider})",
			minutes: {
				one: "{count} minute on your key this month",
				other: "{count} minutes on your key this month",
			},
		},
		sharedLow: {
			one: "The service's shared transcription hours have about {count} minute left until they renew, which may run out before your plan's minutes.",
			other:
				"The service's shared transcription hours have about {count} minutes left until they renew, which may run out before your plan's minutes.",
		},
		sharedOut:
			"The service has used up its shared transcription hours; they reopen on {date}. Your plan's minutes are kept.",
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
