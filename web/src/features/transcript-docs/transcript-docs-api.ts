import { activeTranslator } from "../../i18n/active-translator";
import { apiFetch, apiJson } from "../../lib/api";
import type { Sentence } from "../transcript/transcript-api";

/**
 * A transcript is one recording session: every segment an iPhone uploaded
 * (`s:<sessionId>`) or one browser recording (`f:<fileId>`). The backend
 * joins the machine transcripts of the files with the user's title and
 * edited sentences at read time.
 */
export type SegmentStatus = "none" | "queued" | "running" | "done" | "failed";

export interface TranscriptSummary {
	key: string;
	/** Set by the user; null = the default name. */
	title: string | null;
	/** File name of a one-file recording; null for an iPhone session. */
	name: string | null;
	createdAt?: string | null;
	durationS: number;
	segments: number;
	done: number;
	failed: number;
	/** Queued or running. */
	pending: number;
	/** Segments never sent for transcription (uploaded later, or skipped). */
	untranscribed: number;
	preview: string | null;
}

export interface TranscriptSegment {
	fileId: string;
	/** Position in the session (`<sessionId>#<index>`); 0 for one file. */
	index: number;
	/** Seconds of the session before this segment. */
	offsetS: number;
	durationS: number;
	hasVideo: boolean;
	status: SegmentStatus;
	progress: number;
	error: string | null;
	/** Run that produced `sentences`; edits are saved against it. */
	runId: string | null;
	/** Sentences carry the user's edits. */
	edited: boolean;
	/** Sentences that differ from the machine text: what transcribing again would lose. */
	editedSentences: number;
	/** Times are seconds inside this segment's own file. */
	sentences: Sentence[];
}

/** A decision of the meeting; `at` = seconds from the start of the session. */
export interface SummaryDecision {
	text: string;
	at: number;
}

/** A task someone took on; owner and due date only when they were said. */
export interface SummaryAction {
	text: string;
	owner: string | null;
	due: string | null;
	at: number;
}

/** Minutes of the meeting ("Biên bản"), made on request by the backend. */
export interface TranscriptMinutes {
	model: string;
	createdAt: string;
	summary: string;
	decisions: SummaryDecision[];
	actions: SummaryAction[];
	/** The transcript changed since (edits, transcribed again): worth redoing. */
	stale: boolean;
}

/** "Đánh dấu" pressed while recording; `at` = seconds from the session start. */
export interface Bookmark {
	id: string;
	at: number;
	label: string | null;
}

/** Language a transcript is translated into. */
export type TranslationLang = "en" | "vi";

/**
 * Latest translation run of one language. `failed` also covers a run cut
 * short by a server restart; translating again finishes what is left.
 */
export type TranslationStatus = "none" | "running" | "done" | "failed";

/** A stored translation of the transcript (`GET /api/transcript-docs/:key`). */
export interface TranslationState {
	lang: TranslationLang;
	status: TranslationStatus;
}

/** Progress of the latest run; `done`/`total` count sentences of that run. */
export interface TranslationProgress extends TranslationState {
	done: number;
	total: number;
	error: string | null;
}

export interface TranslatedSentence {
	text: string;
	/** The sentence was edited (or transcribed again) after it was translated. */
	outdated: boolean;
}

/**
 * The translation lined up with the reader: per segment, one entry per
 * sentence of `TranscriptSegment.sentences` (null = not translated, or an
 * empty sentence).
 */
export interface TranscriptTranslation extends TranslationProgress {
	model: string | null;
	updatedAt: string | null;
	/** Sentences whose translation is outdated. */
	outdated: number;
	/** Sentences with no translation yet (transcribed after the last run). */
	missing: number;
	segments: {
		fileId: string;
		sentences: (TranslatedSentence | null)[];
	}[];
}

export interface TranscriptDetail {
	key: string;
	title: string | null;
	name: string | null;
	createdAt?: string | null;
	segments: TranscriptSegment[];
	/** Segment numbers missing between the ones present (iPhone sessions). */
	missingIndexes: number[];
	/** Null until "Tóm tắt" is pressed (absent from older backends). */
	summary?: TranscriptMinutes | null;
	/** Sorted by `at`. */
	bookmarks?: Bookmark[];
	/** Stored translations (absent from older backends). */
	translations?: TranslationState[];
	/** Display name of each speaker id found in the sentences (absent from older backends). */
	speakers?: { names: Record<string, string> } | null;
}

export interface SegmentRunStatus {
	fileId: string;
	status: SegmentStatus;
	progress: number;
	runId: string | null;
}

/** Key of the transcript a recording belongs to (same rule as the backend). */
export function transcriptKey(
	scope: { session: string } | { fileId: string },
): string {
	return "session" in scope ? `s:${scope.session}` : `f:${scope.fileId}`;
}

function docPath(key: string, rest = ""): string {
	return `/api/transcript-docs/${encodeURIComponent(key)}${rest}`;
}

export async function listTranscriptDocs(
	fetcher: typeof fetch = fetch,
): Promise<TranscriptSummary[]> {
	const body = await apiJson<{ items?: TranscriptSummary[] }>(
		"/api/transcript-docs",
		{ fallbackError: activeTranslator().t("errors.transcripts.list") },
		fetcher,
	);
	return body.items ?? [];
}

export function fetchTranscriptDoc(
	key: string,
	fetcher: typeof fetch = fetch,
): Promise<TranscriptDetail> {
	return apiJson<TranscriptDetail>(
		docPath(key),
		{ fallbackError: activeTranslator().t("errors.transcripts.doc") },
		fetcher,
	);
}

/** Status of every segment, without sentences: what to poll while transcribing. */
export async function fetchTranscriptStatus(
	key: string,
	fetcher: typeof fetch = fetch,
): Promise<SegmentRunStatus[]> {
	const body = await apiJson<{ items?: SegmentRunStatus[] }>(
		docPath(key, "/status"),
		{ fallbackError: activeTranslator().t("errors.transcripts.progress") },
		fetcher,
	);
	return body.items ?? [];
}

/** Longest title the backend accepts, in characters. */
export const MAX_TITLE_CHARS = 120;

/** What the backend stores for a title: one line, spaces collapsed; null = the default name. */
export function normalizeTitle(title: string): string | null {
	return title.split(/\s+/).filter(Boolean).join(" ") || null;
}

/** Renames; an empty title goes back to the default name. */
export async function renameTranscript(
	key: string,
	title: string,
	fetcher: typeof fetch = fetch,
): Promise<string | null> {
	const body = await apiJson<{ title?: string | null }>(
		docPath(key),
		{
			method: "PATCH",
			json: { title: normalizeTitle(title) },
			fallbackError: activeTranslator().t("errors.transcripts.rename"),
		},
		fetcher,
	);
	return body.title ?? null;
}

/**
 * Renames speakers everywhere in the transcript (reader, downloads,
 * minutes); an empty name goes back to "Người N". Returns every name.
 */
export async function renameSpeakers(
	key: string,
	names: Readonly<Record<string, string>>,
	fetcher: typeof fetch = fetch,
): Promise<Record<string, string>> {
	const json: Record<string, string | null> = {};
	for (const [id, name] of Object.entries(names))
		json[id] = name.trim() || null;
	const body = await apiJson<{ names?: Record<string, string> }>(
		docPath(key, "/speakers"),
		{
			method: "PATCH",
			json: { names: json },
			fallbackError: activeTranslator().t("errors.transcripts.speakers"),
		},
		fetcher,
	);
	return body.names ?? {};
}

/**
 * Saves the edited sentences of one segment. The backend answers 409 when
 * the segment was transcribed again since `runId` (the edits would land on
 * other sentences).
 */
export async function saveSegmentTexts(
	key: string,
	fileId: string,
	runId: string,
	texts: readonly string[],
	fetcher: typeof fetch = fetch,
): Promise<void> {
	await apiFetch(
		docPath(key, `/segments/${encodeURIComponent(fileId)}`),
		{
			method: "PUT",
			json: { runId, texts },
			fallbackError: activeTranslator().t("errors.transcripts.saveSegment"),
		},
		fetcher,
	);
}

/** Deletes the transcript and its edits; the audio stays. */
export async function deleteTranscript(
	key: string,
	fetcher: typeof fetch = fetch,
): Promise<void> {
	await apiFetch(
		docPath(key),
		{
			method: "DELETE",
			fallbackError: activeTranslator().t("errors.transcripts.delete"),
		},
		fetcher,
	);
}

/**
 * Makes (or makes again) the minutes of the transcript. Refusals carry a
 * code: `NOT_TRANSCRIBED` (409), `SUMMARY_DAILY_LIMIT` / `CAP_REACHED`
 * (403, with the counts and `retryAt`).
 */
export function createSummary(
	key: string,
	fetcher: typeof fetch = fetch,
): Promise<TranscriptMinutes> {
	return apiJson<TranscriptMinutes>(
		docPath(key, "/summary"),
		{
			method: "POST",
			fallbackError: activeTranslator().t("errors.transcripts.summary"),
		},
		fetcher,
	);
}

/** Sends the minutes to the user's private chat with the Scribe bot. */
export async function shareSummary(
	key: string,
	fetcher: typeof fetch = fetch,
): Promise<void> {
	await apiFetch(
		docPath(key, "/share"),
		{
			method: "POST",
			fallbackError: activeTranslator().t("errors.transcripts.share"),
		},
		fetcher,
	);
}

/**
 * Saves a bookmark; works while recording, before any file is stored. `id`
 * (24 hex) is made by the caller: sending it again returns the bookmark
 * already saved instead of adding a second one.
 */
export function addBookmark(
	key: string,
	at: number,
	id: string,
	fetcher: typeof fetch = fetch,
): Promise<Bookmark> {
	return apiJson<Bookmark>(
		docPath(key, "/bookmarks"),
		{
			method: "POST",
			json: { id, at },
			fallbackError: activeTranslator().t("errors.transcripts.bookmark"),
		},
		fetcher,
	);
}

export async function deleteBookmark(
	key: string,
	id: string,
	fetcher: typeof fetch = fetch,
): Promise<void> {
	await apiFetch(
		docPath(key, `/bookmarks/${encodeURIComponent(id)}`),
		{
			method: "DELETE",
			fallbackError: activeTranslator().t("errors.transcripts.deleteBookmark"),
		},
		fetcher,
	);
}

/** The translation into `lang`, lined up with the reader's sentences. */
export function fetchTranslation(
	key: string,
	lang: TranslationLang,
	fetcher: typeof fetch = fetch,
): Promise<TranscriptTranslation> {
	return apiJson<TranscriptTranslation>(
		docPath(key, `/translation?lang=${lang}`),
		{ fallbackError: activeTranslator().t("errors.transcripts.translation") },
		fetcher,
	);
}

/** Progress of the latest run into `lang`, without sentences: what to poll. */
export function fetchTranslationStatus(
	key: string,
	lang: TranslationLang,
	fetcher: typeof fetch = fetch,
): Promise<TranslationProgress> {
	return apiJson<TranslationProgress>(
		docPath(key, `/translation/status?lang=${lang}`),
		{
			fallbackError: activeTranslator().t(
				"errors.transcripts.translationProgress",
			),
		},
		fetcher,
	);
}

/**
 * Translates the sentences of the transcript that have no translation into
 * `lang` yet, or an outdated one. Answers `running` (202) while the server
 * works through them, or `done` with `total: 0` when nothing was left.
 * Refusals carry a code: `NOT_TRANSCRIBED` (409), `TRANSLATE_DAILY_LIMIT` /
 * `TRANSLATE_MONTHLY_LIMIT` / `CAP_REACHED` (403, with `retryAt`).
 */
export function startTranslation(
	key: string,
	lang: TranslationLang,
	fetcher: typeof fetch = fetch,
): Promise<TranslationProgress> {
	return apiJson<TranslationProgress>(
		docPath(key, "/translation"),
		{
			method: "POST",
			json: { lang },
			fallbackError: activeTranslator().t("errors.transcripts.translate"),
		},
		fetcher,
	);
}
