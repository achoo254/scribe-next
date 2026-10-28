import type { SourceMessages, Translation } from "../../message-types";

/** English of `vi/errors.ts`. */
export const errors: Translation<SourceMessages["errors"]> = {
	network: "Couldn't reach the server. Check your connection and try again.",
	requestFailed: "Request failed",
	generic: "Something went wrong.",
	files: {
		history: "Couldn't load the recording history",
		list: "Couldn't load the file list",
		delete: "Couldn't delete the file",
		deleteAll: "Couldn't delete the recording history",
		usage: "Couldn't load storage usage",
		download: "Couldn't download the file",
	},
	transcripts: {
		request: "Couldn't send the transcription request",
		load: "Couldn't load the transcript",
		list: "Couldn't load the transcript list",
		doc: "Couldn't load the transcript",
		progress: "Couldn't load the transcription progress",
		rename: "Couldn't rename",
		speakers: "Couldn't rename the speakers",
		saveSegment: "Couldn't save the edited sentence",
		delete: "Couldn't delete the transcript",
		summary: "Couldn't summarise",
		share: "Couldn't send to chat",
		bookmark: "Couldn't save the bookmark",
		deleteBookmark: "Couldn't delete the bookmark",
		translate: "Couldn't translate",
		translation: "Couldn't load the translation",
		translationProgress: "Couldn't load the translation progress",
	},
	search: {
		search: "Couldn't search the transcripts",
		ask: "Couldn't ask Scribe",
	},
	keys: {
		load: "Couldn't load your API keys",
		save: "Couldn't save the key",
		delete: "Couldn't delete the key",
		prefs: "Couldn't change the choice",
	},
	chat: {
		status: "Couldn't check the chat link",
		link: "Couldn't create the link",
		unlink: "Couldn't unlink chat",
	},
	split: {
		create: "Couldn't create the split job",
		progress: "Couldn't get the job progress",
		failed: "Splitting failed",
		watch: "Lost track of the job progress",
	},
	server: {
		quotaExceeded:
			"Not enough transcription minutes: you've used {used}/{limit} minutes this month.",
		dailyLimit:
			"The free plan transcribes {limit} minutes a day and you've used {used} today. Available again at {time}.",
		tooLongForFree:
			"The free plan transcribes up to {limit} minutes a day; the selected recordings are {used} minutes long. Choose fewer recordings.",
		accountTooNew: "New accounts can use free transcription from {time}.",
		providerHours:
			"This month's transcription hours are used up. Available again at {time}.",
		notTranscribed:
			"No part of this recording is transcribed yet. Transcribe it first.",
		confirmRequired: "Type the confirmation word to delete your account.",
		chatLinkDisabled: "Chat linking is not enabled on this server.",
		chatNotLinked: "chat.hoandat.com is not linked yet.",
		chatBlocked:
			"Couldn't send because the Scribe bot is blocked in chat. The link was removed; link chat again.",
		noSummary: "No minutes yet. Press Summarise first, then send.",
		bookmarksFull: "This recording has the maximum number of bookmarks.",
		realtimeNotAllowed: "This account can't use live transcription yet.",
		userKeyFailed:
			"Your own API key couldn't be used. Check it in Your API keys.",
		keyFormat: "The key doesn't match the provider's format.",
		keyRejected: "The provider rejected this key.",
		keyNoCredit: "The account of this key is out of credit.",
		keyCheckFailed: "Couldn't check the key right now. Try again later.",
		accountDeleted: "This Scribe account has been deleted.",
		signedOut: "Your session has expired. Sign in again.",
		tooLarge:
			"The file is too large or your storage is full. Delete some recordings or files to keep recording and uploading.",
		tooMany: "Too many requests. Wait a moment and try again.",
		unavailable: "This feature is paused for now. Try again later.",
	},
	upload: {
		status: "Couldn't get the upload status",
		chunk: "Upload failed. You can try again",
		noProgress: "The server stopped receiving data. Try uploading again.",
		complete: "Couldn't finish the upload",
		init: "Couldn't start the upload (file too large or over your limit)",
		failed: "Upload failed",
	},
};
