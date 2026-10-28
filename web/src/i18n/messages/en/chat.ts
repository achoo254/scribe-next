import type { SourceMessages, Translation } from "../../message-types";

/**
 * English of `vi/chat.ts`. The bot's confirm button is written by the
 * Scribe server in Vietnamese, so step 3 names it as the user will see it.
 */
export const chat: Translation<SourceMessages["chat"]> = {
	menu: {
		link: "Link chat.hoandat.com",
		unlink: "Unlink chat",
		unlinked: "Chat unlinked",
		unlinkFailed: "Couldn't unlink chat",
		linkedSince: "Chat linked at {time}",
		linkedWith: "Linked with {name}",
	},
	dialog: {
		title: "Link chat.hoandat.com",
		intro:
			"Link once so Scribe can send minutes to your private chat with the Scribe bot on chat.hoandat.com.",
		step1: "Press “Open chat” (your browser opens chat.hoandat.com).",
		step2:
			"Sign in if asked, then send /start in the chat with the Scribe bot.",
		step3:
			"The bot replies with your email: press “Xác nhận liên kết” (Confirm link).",
		step4: "Come back here: Scribe notices by itself when the link is done.",
		steps: "Steps to link",
		expires: "The link works once and expires at {time}.",
		preparing: "Creating the link…",
		open: "Open chat",
		opensBrowser: ", opens in the browser",
		waiting: "Waiting for you to send /start and confirm the link in chat…",
		linked: "Linked. Minutes will go to your private chat with the Scribe bot.",
		linkedTitle: "Chat linked",
	},
};
