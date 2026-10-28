import type { SourceMessages, Translation } from "../../message-types";

/** English of `vi/account.ts`. */
export const account: Translation<SourceMessages["account"]> = {
	menu: "Delete account",
	delete: {
		title: "Delete Scribe account",
		question: "Permanently delete the Scribe account of {email}?",
		intro: "Scribe deletes from the server right away:",
		files: "Every recording, uploaded file and split part.",
		transcripts:
			"Transcripts, edited sentences, speaker names, minutes, bookmarks, translations and the search index.",
		keys: "Your API keys and the chat.hoandat.com link.",
		sessions:
			"Sign-in sessions on every device and the account's usage figures.",
		undo: "This cannot be undone. Download anything you want to keep first.",
		audit:
			"Security logs (time, action, IP address) are kept for up to 180 days, without your email or file names; cost figures are kept anonymised.",
		idp: "Your {idpName} account is shared by several apps and is not deleted here. To delete it too, go to <account>auth.hoandat.com/account</account>.",
		busy: "A recording is running or a file is still uploading. Stop recording and wait for the upload to finish, then delete your account.",
		confirmWord: "DELETE",
		confirmLabel: "Type {word} to confirm",
		confirm: "Delete account",
		recordingOpen: "A recording is running. Stop recording and try again.",
	},
};
