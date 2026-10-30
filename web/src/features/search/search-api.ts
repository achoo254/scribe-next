import { activeTranslator } from "../../i18n/active-translator";
import { apiJson } from "../../lib/api";

/**
 * One sentence of the user's transcripts matching a search (`GET
 * /api/search`). `at` = seconds from the start of the session, like the
 * minutes; `before`/`after` are the sentences around it.
 */
export interface SearchResult {
	key: string;
	/** Title the user set, or the file name of a one-file recording; null for an unnamed iPhone session. */
	title: string | null;
	/** Start of the session. */
	createdAt: string | null;
	fileId: string;
	at: number;
	text: string;
	before: string | null;
	after: string | null;
}

export interface SearchReply {
	results: SearchResult[];
	/** Every matching sentence, beyond the `results` sent. */
	total: number;
}

/** A source of an answer: `[n]` in the answer is citation `n`. */
export interface AskCitation {
	n: number;
	key: string;
	title: string | null;
	createdAt: string | null;
	at: number;
	text: string;
}

export interface AskReply {
	answer: string;
	citations: AskCitation[];
}

/** Longest question the backend takes (characters). */
export const MAX_QUESTION_CHARS = 500;

/** Sentences matching `query` (with or without Vietnamese marks), most relevant first. */
export function searchTranscripts(
	query: string,
	{ key, limit }: { key?: string; limit?: number } = {},
	signal?: AbortSignal,
	fetcher: typeof fetch = fetch,
): Promise<SearchReply> {
	const params = new URLSearchParams({ q: query });
	if (key) params.set("key", key);
	if (limit) params.set("limit", String(limit));
	return apiJson<SearchReply>(
		`/api/search?${params.toString()}`,
		{ signal, fallbackError: activeTranslator().t("errors.search.search") },
		fetcher,
	);
}

/**
 * Asks Scribe about every transcript, or only `key`; the answer comes in
 * the language on screen. Refusals carry a code:
 * `ASK_DAILY_LIMIT` / `ASK_MONTHLY_LIMIT` / `CAP_REACHED` (403 with the
 * counts and `retryAt`), `NOT_TRANSCRIBED` (409).
 */
export function askScribe(
	question: string,
	key?: string,
	fetcher: typeof fetch = fetch,
): Promise<AskReply> {
	return apiJson<AskReply>(
		"/api/ask",
		{
			method: "POST",
			json: {
				question,
				...(key ? { key } : {}),
				lang: activeTranslator().locale,
			},
			fallbackError: activeTranslator().t("errors.search.ask"),
		},
		fetcher,
	);
}

/** Text and `[n]` citations of an answer, in order. */
export type AnswerPart =
	| { kind: "text"; text: string }
	| { kind: "cite"; citation: AskCitation };

/**
 * Splits an answer at its `[n]` markers; a marker whose citation is
 * missing stays plain text.
 */
export function answerParts(reply: AskReply): AnswerPart[] {
	const byNumber = new Map(reply.citations.map((c) => [c.n, c]));
	const parts: AnswerPart[] = [];
	let at = 0;
	for (const match of reply.answer.matchAll(/\[(\d{1,3})\]/g)) {
		const citation = byNumber.get(Number(match[1]));
		if (!citation) continue;
		const start = match.index;
		if (start > at)
			parts.push({ kind: "text", text: reply.answer.slice(at, start) });
		parts.push({ kind: "cite", citation });
		at = start + match[0].length;
	}
	if (at < reply.answer.length) {
		parts.push({ kind: "text", text: reply.answer.slice(at) });
	}
	return parts;
}
