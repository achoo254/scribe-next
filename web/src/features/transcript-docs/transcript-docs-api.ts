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

export interface TranscriptDetail {
	key: string;
	title: string | null;
	name: string | null;
	createdAt?: string | null;
	segments: TranscriptSegment[];
	/** Segment numbers missing between the ones present (iPhone sessions). */
	missingIndexes: number[];
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
			json: { title: title.trim() || null },
			fallbackError: activeTranslator().t("errors.transcripts.rename"),
		},
		fetcher,
	);
	return body.title ?? null;
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
