import type { SourceMessages, Translation } from "../../message-types";

/** English of `vi/search.ts`: search and “Ask Scribe”. */
export const search: Translation<SourceMessages["search"]> = {
	entry: {
		label: "Search transcripts",
		placeholder: "E.g. hop dong",
		submit: "Search",
	},
	page: {
		back: "Transcripts",
		title: "Search recordings",
		description:
			"Find sentences in all your transcripts; Vietnamese with or without accents both work. Press a sentence to play from there.",
		field: "Search transcripts",
		placeholder: "E.g. hợp đồng thuê nhà",
	},
	results: {
		title: "Results",
		count: {
			one: "{count} matching sentence",
			other: "{count} matching sentences",
		},
		showing: "{count} best matches of {total}",
		searching: "Searching…",
		idle: "Type a few words to search your transcripts.",
		tooShort: "Type at least 2 characters to search.",
		emptyTitle: "No matching sentences",
		emptyBody:
			"Try other or fewer words. Only transcribed sentences are searched.",
		failed: "Search failed",
	},
	ask: {
		title: "Ask Scribe",
		introAll:
			"Ask in plain language across all your recordings. Answers are based only on the recordings and cite timestamps to play back.",
		introOne:
			"Ask about this recording in plain language. Answers are based only on the recording and cite timestamps to play back.",
		field: "Question for Scribe",
		placeholderAll: "E.g. What price did the client agree to?",
		placeholderOne: "E.g. Who will send the quote?",
		submit: "Ask",
		running: "Looking for the answer… this can take a few dozen seconds.",
		failed: "Couldn't answer",
		question: "You asked: {question}",
		sources: "Sources",
		cite: "Source {n}: {title}, play from {time}",
		session: "Questions are kept only until you leave the page.",
		dailyLimit:
			"You've used {used}/{limit} questions today. Available again at {time}.",
		monthlyLimit:
			"This account has used its summary and question allowance this month ({used}/{limit} equivalent minutes). Available again at {time}.",
		capReached:
			"Questions have used this month's shared allowance. Available again at {time}.",
	},
};
