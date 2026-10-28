import type { SourceMessages, Translation } from "../../message-types";

/** English of `vi/login.ts`. */
export const login: Translation<SourceMessages["login"]> = {
	title: "Sign in",
	subtitle: "Record, split and transcribe with your {idpName} account.",
	sessionCheck: "Checking your session…",
	noSession: "You signed in but the server has no session yet. Try again.",
	errors: {
		denied: "You declined the sign-in on the account page.",
		expired: "The sign-in expired. Press Sign in again.",
		invalid: "The sign-in reply was not valid. Press Sign in again.",
		unverified:
			"The account's email is not verified. Sign in with a Google or GitHub account whose email is verified.",
		forbidden: "This account may not use Scribe yet.",
		deleting:
			"This Scribe account was just deleted and its data is being removed. Try signing in again in a few minutes.",
		idp: "The sign-in server reported an error. Try again later.",
		busy: "Too many sign-ins. Wait a minute and try again.",
		failed: "Sign-in failed. Try again later.",
	},
	failed: "Sign-in failed",
	accountDeleted:
		"Your Scribe account and all your data are deleted. Your {idpName} account remains at auth.hoandat.com.",
	accountDeletedPending:
		"Your Scribe account is deleted. The rest of your data will be removed within an hour.",
};
