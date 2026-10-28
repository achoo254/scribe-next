import type { SourceMessages, Translation } from "../../message-types";

/** English of `vi/record.ts`: the Record page and the recording history. */
export const record: Translation<SourceMessages["record"]> = {
	title: "Record",
	description:
		"Record the microphone, system audio or the screen into one recording. It uploads while you record and shows up in Recording history below.",
	nativeDescription:
		"Record with the iPhone microphone, even when the screen is locked or a call comes in. Each finished part uploads to Recording history while recording continues.",
	noSource: "Turn on at least one source.",
	sourcesHelp:
		"System audio and screen recording need screen sharing, which works best in <b>Chrome/Edge</b>: choose “Entire screen” and turn on “Share audio” to get system audio. Firefox/Safari may not support system audio. Several sources → mixed into one recording.",
	noScreenCapture: "This browser can't share the screen (microphone only).",
	sources: {
		legend: "Sources",
		mic: "Microphone",
		systemAudio: "System audio",
		screen: "Include the screen (video)",
	},
	upload: {
		choose: "Upload a file",
		hint: "An audio or video file on your device, to play back and transcribe.",
		progress: "Upload progress",
		uploading: "Uploading “{name}”… {percent}%",
		resume: "Resume upload",
		done: "Uploaded “{name}”. Play and transcribe it in <b>Recording history</b> below.",
	},
	splitAfter: {
		toggle: "Split into parts by minutes",
		minutes: "Minutes per part",
		invalid: "Enter a number of minutes above 0.",
	},
	live: {
		level: "Audio level",
		monitor: "Entire screen",
		window: "Window",
		browser: "Browser tab",
	},
	start: "Start recording",
	stop: "Stop recording",
	pause: "Pause",
	resume: "Resume",
	timer: "Recording — {clock}",
	timerPaused: "Paused — {clock}",
	saving: "Saving the recording…",
	splitting: "Splitting the recording into parts…",
	splitDone: {
		one: "Split into {count} part. See it in <b>Recording history</b> below.",
		other:
			"Split into {count} parts. See them in <b>Recording history</b> below.",
	},
	bookmark: {
		mark: "Bookmark",
		marked: { one: "{count} bookmark", other: "{count} bookmarks" },
		waiting: {
			one: "{count} bookmark waiting to save, retrying",
			other: "{count} bookmarks waiting to save, retrying",
		},
		failed: {
			one: "{count} bookmark couldn't be saved: {reason}",
			other: "{count} bookmarks couldn't be saved: {reason}",
		},
		dropped: {
			one: "{count} bookmark dropped because the recording was split or deleted",
			other:
				"{count} bookmarks dropped because the recording was split or deleted",
		},
		unknownReason: "unknown reason",
	},
	native: {
		segmentLength: "Part length",
		segmentOption: { one: "{count} minute", other: "{count} minutes" },
		uploadsWaiting: {
			one: "Waiting to upload: {count} part (kept on this device, uploads when online).",
			other:
				"Waiting to upload: {count} parts (kept on this device, upload when online).",
		},
		uploadsFailed: {
			one: "{count} part was refused by the server and is kept on this device.",
			other:
				"{count} parts were refused by the server and are kept on this device.",
		},
		realtime: "Live transcription",
		realtimeHint:
			"Sends audio to the transcription service so text appears right after each sentence; counts toward your transcription minutes. The full transcript still runs after upload.",
		liveTitle: "Transcribing",
		liveTitleDone: "Live transcription",
		liveNote: "Preview. The full transcript runs after upload.",
		liveConnecting: "Connecting…",
		liveEmpty: "Text appears here after each sentence.",
		liveDisconnected: "Disconnected since {clock}, reconnecting…",
		liveGap: "{from}–{to} · missed, wait for the full transcript",
	},
	groups: {
		expiresSoon: "deleted soon",
		expiresIn: {
			one: "deleted in {count} day",
			other: "deleted in {count} days",
		},
		iphoneSession: "iPhone recording",
		today: "Today",
		yesterday: "Yesterday",
	},
	history: {
		title: "Recording history",
		downloadAll: "Download all",
		loading: "Loading recording history…",
		loadFailed: "Couldn't load the recording history",
		emptyTitle: "No recordings yet",
		emptyBody: "Recordings show up here after you press “Stop recording”.",
		retention: {
			one: "Recordings are <b>deleted automatically after {count} day</b> from creation. Download them to keep them longer.",
			other:
				"Recordings are <b>deleted automatically after {count} days</b> from creation. Download them to keep them longer.",
		},
		dayMeta: {
			one: "{count} recording · {duration}",
			other: "{count} recordings · {duration}",
		},
		downloadDay: "Download day",
		downloadDayLabel: "Download day {date}",
		deleted: "Deleted “{name}”",
		undo: "Undo",
		deleteAll: "Delete all",
		deleteAllTitle: "Delete the whole recording history",
		deleteAllQuestion: {
			one: "Delete {count} file, {size} in total?",
			other: "Delete {count} files, {size} in total?",
		},
		deleteAllBody:
			"Every recording and uploaded file in this list, with its transcripts, minutes and bookmarks, will be deleted permanently. This cannot be undone. Kept: the recording in progress (an iPhone recording with a new part in the last 10 minutes counts as in progress) and recordings being split.",
		deletedAll: {
			one: "Deleted {count} file",
			other: "Deleted {count} files",
		},
		deletedAllSkipped: {
			one: "Kept {count} file of a recording in progress or being split.",
			other: "Kept {count} files of recordings in progress or being split.",
		},
	},
	row: {
		screen: "Screen",
		mic: "Mic",
		upload: "Uploaded file",
		delete: "Delete {name}",
		play: "Play",
		hide: "Hide",
		menu: "Actions for {name}",
		download: "Download",
		saveToFiles: "Save to Files",
	},
	rename: {
		edit: "Rename {name}",
		field: "Recording name",
		hint: "Enter to save, Esc to cancel. Leave empty for the default name.",
	},
	session: {
		badge: "iPhone",
		meta: {
			one: "{count} part · {duration} · {size}",
			other: "{count} parts · {duration} · {size}",
		},
		expand: "Show parts",
		collapse: "Collapse",
		part: "Part {number}",
		delete: "Delete the whole session {name}",
	},
};
