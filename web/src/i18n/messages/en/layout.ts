import type { SourceMessages, Translation } from "../../message-types";

/** English of `vi/layout.ts`. */
export const layout: Translation<SourceMessages["layout"]> = {
	skipLink: "Skip to main content",
	breadcrumb: "Breadcrumb",
	appHeader: {
		home: "Scribe, go to Record",
		nav: "Sections",
		record: "Record",
		// "Split file" (the page title) does not fit the phone's nav row.
		split: "Split",
		transcript: "Transcripts",
	},
	accountMenu: {
		trigger: "{email} · Account",
		signOut: "Sign out",
	},
	idpMenu: {
		account: "{idpName} account",
		apps: "Apps",
		opensIdp: ", opens the {idpName} page in a new tab",
	},
	signOutConfirm: {
		title: "Sign out",
		question: "Sign out before your work is done?",
		recording: "The running recording will stop; what was recorded is kept.",
		segmentsWaiting: {
			one: "{count} part not yet uploaded stays on this device and uploads when you sign in again.",
			other:
				"{count} parts not yet uploaded stay on this device and upload when you sign in again.",
		},
		fileUploading: "The file uploading in Split file will stop halfway.",
		confirm: "Sign out anyway",
	},
	footer: {
		links: "Account links",
		ecosystem: "An app from hoandat.com",
	},
	leaveGuard: {
		title: "Leave page",
		question: "Leave without saving?",
		detail: "Unsaved edits will be lost.",
		confirm: "Leave anyway",
	},
	pageError: {
		title: "This page hit an error",
		body: "A running recording (if any) carries on. Open another page or reload this one.",
		reload: "Reload page",
	},
};
