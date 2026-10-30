import type { SourceMessages, Translation } from "../../message-types";

/** English of `vi/transcript.ts`: the Transcripts pages and transcribing. */
export const transcript: Translation<SourceMessages["transcript"]> = {
	defaultName: "Recording",
	progress: "Transcribing… {percent}%",
	status: {
		running: "Transcribing {done}/{sent}",
		failed: { one: "{count} part failed", other: "{count} parts failed" },
		untranscribed: {
			one: "{count} part not transcribed",
			other: "{count} parts not transcribed",
		},
		done: "Transcribed",
	},
	transcribe: {
		failed: "Couldn't transcribe",
		confirmTitle: "Transcribe a long recording",
		send: "Transcribe",
		usage: "This recording uses about <strong>{minutes} minutes</strong>.",
		usageOfLeft:
			"This recording uses about <strong>{minutes} minutes</strong> of the <strong>{left} minutes</strong> left this month.",
		notEnough: "Not enough minutes left for this recording.",
	},
	download: {
		menu: "Download",
		language: "Language",
		original: "Original",
	},
	translate: {
		menu: "Translate",
		menuActive: "Translation: {language}",
		languages: {
			en: "English",
			vi: "Vietnamese",
			"zh-CN": "Chinese (Simplified)",
		},
		view: {
			legend: "View",
			original: "Original",
			translation: "Translation",
			both: "Side by side",
		},
		running: "Translating into {language}… {done}/{total} sentences",
		starting: "Starting translation…",
		failed: "Couldn't translate",
		resume: "Resume translation",
		outdated: {
			one: "{count} sentence was edited after translation.",
			other: "{count} sentences were edited after translation.",
		},
		missing: {
			one: "{count} sentence has no translation yet.",
			other: "{count} sentences have no translation yet.",
		},
		retranslate: "Retranslate edited sentences",
		fillMissing: "Translate the rest",
		outdatedMark: "old translation",
		dailyLimit:
			"You've used {used}/{limit} translations today. Available again at {time}.",
		monthlyLimit:
			"This account has used its summary, question and translation allowance this month ({used}/{limit} equivalent minutes). Available again at {time}.",
		capReached:
			"Translation has used this month's shared allowance. Available again at {time}.",
	},
	panel: {
		done: "Transcribed",
		partial: "Transcribed {done}/{total} parts",
		open: "Open transcript",
		hideText: "Hide text",
		showText: "Show text",
		resume: "Transcribe the rest",
		rerun: "Transcribe again",
		failed: "Transcription failed.",
		checkEditsFailed: "Couldn't check for edits",
		rerunQuestion: "Discard edits in {segments} parts ({sentences} sentences)?",
		rerunBody:
			"Transcribing the whole recording again replaces every sentence with the new transcript. To keep your edits, transcribe parts one at a time on the Transcripts page.",
		rerunNamesQuestion: {
			one: "Reset the name of {count} renamed speaker?",
			other: "Reset the names of {count} renamed speakers?",
		},
	},
	segment: {
		none: "This part is not transcribed yet.",
		queued: "Waiting to transcribe…",
		running: "Transcribing… {percent}%",
		failed: "Transcription error: {reason}",
		unknownReason: "unknown reason",
		empty: "No speech in this part.",
		heading: "Part {number} · {offset}",
		editedSuffix: " · edited",
		edited: "Edited",
		rerun: "Transcribe this part again",
		rerunAll: "Transcribe the whole session again",
		sentence: "Sentence {number}",
		stale:
			"This part was just transcribed again, so your edit no longer matches. Cancel and edit the new version.",
	},
	list: {
		title: "Transcripts",
		description:
			"Transcripts of your recordings, newest first. Open one to read it, play from any sentence and fix the text.",
		loading: "Loading transcripts…",
		loadFailed: "Couldn't load the transcript list",
		emptyTitle: "No transcripts yet",
		emptyBody:
			"Press “Transcribe” on a recording on the Record page; the transcript will show up here.",
		emptyAction: "Open Record",
		segments: { one: "{count} part", other: "{count} parts" },
		actions: "Actions for “{name}”",
		dayMeta: {
			one: "{count} transcript · {duration}",
			other: "{count} transcripts · {duration}",
		},
	},
	quota: {
		paused:
			"Transcription is paused and the admin has been told. Try again later.",
		capReached:
			"Transcription has used this month's shared allowance and reopens at the start of next month.",
	},
	minutes: {
		title: "Minutes",
		intro:
			"Summarise the transcript into minutes: key points, decisions and action items, each with a timestamp to play back.",
		needsTranscript: "Transcribe at least one part to summarise.",
		create: "Summarise",
		recreate: "Summarise again",
		running: "Summarising… this can take up to a minute.",
		stale: "Outdated",
		staleBody:
			"The transcript changed after this summary. Press “Summarise again” for new minutes.",
		decisions: "Decisions",
		noDecisions: "No decisions were mentioned.",
		actions: "Action items",
		noActions: "No action items were mentioned.",
		owner: "Owner: {owner}",
		due: "Due: {due}",
		createdAt: "Created {date}",
		jump: "Play from {time}",
		dailyLimit:
			"You've used {used}/{limit} summaries today. Available again at {time}.",
		monthlyLimit:
			"This account has used its summary and question allowance this month ({used}/{limit} equivalent minutes). Available again at {time}.",
		capReached:
			"Summaries have used this month's shared allowance. Available again at {time}.",
		failed: "Couldn't summarise",
		send: "Send to chat",
		link: "Link chat",
		sent: "Minutes sent to chat",
		sendFailed: "Couldn't send the minutes",
	},
	bookmarks: {
		title: "Bookmarks",
		count: { one: "{count} bookmark", other: "{count} bookmarks" },
		unnamed: "Bookmark {number}",
		inline: "Bookmark {time}",
		inlineLabel: "Bookmark {time} · {label}",
		delete: "Delete bookmark {time}",
		deleted: "Bookmark deleted",
		deleteFailed: "Couldn't delete the bookmark",
	},
	speakers: {
		open: "Speakers",
		title: "Speaker names",
		description:
			"Names apply to the whole transcript, downloads and minutes. Leave empty for the default name.",
		field: "Name of {name}",
		fallback: "Speaker {number}",
		save: "Save names",
		saved: "Speaker names changed",
		reset:
			"The new transcript identified the speakers again, so names were reset to Speaker 1, Speaker 2…",
	},
	reader: {
		back: "Transcripts",
		loading: "Loading the transcript…",
		errorTitle: "Couldn't open the transcript",
		loadFailed: "Couldn't load the content",
		rename: "Rename",
		titleField: "Transcript name",
		titleHint: "Leave empty for the default name.",
		saveTitle: "Save name",
		renamed: "Transcript renamed",
		menu: "Transcript actions",
		delete: "Delete transcript",
		deleteTitle: "Delete transcript",
		deleteQuestion: "Delete the transcript “{name}”?",
		deleteBody:
			"The text and your edits will be lost. The audio stays in Recording history.",
		deleted: "Deleted the transcript “{name}”",
		unsent: {
			one: "{count} part not transcribed yet.",
			other: "{count} parts not transcribed yet.",
		},
		sendRest: {
			one: "Transcribe the other {count} part",
			other: "Transcribe the other {count} parts",
		},
		missing: "Part {number} missing",
		playing: "Playing part {number}",
		rerunTitle: "Transcribe part again",
		rerunQuestion: {
			one: "Discard {count} edited sentence in part {number}?",
			other: "Discard {count} edited sentences in part {number}?",
		},
		rerunBody:
			"This part gets a new machine transcript; other parts keep your edits.",
		rerunAllTitle: "Transcribe the whole session again",
		rerunAllQuestion: {
			one: "Discard {count} edited sentence in the whole session?",
			other: "Discard {count} edited sentences in the whole session?",
		},
		rerunAllBody:
			"An iPhone session is transcribed again in one go so speaker labels stay consistent, so every part gets a new machine transcript.",
	},
};
