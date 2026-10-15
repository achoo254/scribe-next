import { apiJson } from "../../lib/api";

export type TranscriptStatus = "queued" | "running" | "done" | "failed";

/** A sentence of a transcript; times are seconds inside its own file. */
export interface Sentence {
	start: number;
	end: number;
	text: string;
}

export interface Transcript {
	fileId: string;
	status: TranscriptStatus;
	/** 0–100 while running. */
	progress: number;
	error?: string | null;
	sentences: Sentence[];
}

/** The backend takes at most this many files per request; longer sessions go in batches. */
const BATCH = 200;

function batches<T>(items: readonly T[]): T[][] {
	const out: T[][] = [];
	for (let i = 0; i < items.length; i += BATCH)
		out.push(items.slice(i, i + BATCH));
	return out;
}

/**
 * Queues the recordings for transcription with the user's own OpenAI key.
 * The backend keeps the key in memory only until these files are done.
 */
export async function requestTranscripts(
	fileIds: readonly string[],
	apiKey: string,
	fetcher: typeof fetch = fetch,
): Promise<number> {
	let queued = 0;
	for (const batch of batches(fileIds)) {
		const body = await apiJson<{ queued?: number }>(
			"/api/transcripts",
			{
				method: "POST",
				json: { fileIds: batch, apiKey },
				fallbackError: "Không gửi được yêu cầu chép lời",
			},
			fetcher,
		);
		queued += body.queued ?? 0;
	}
	return queued;
}

/** Transcripts of these files (files never transcribed are left out). */
export async function fetchTranscripts(
	fileIds: readonly string[],
	fetcher: typeof fetch = fetch,
): Promise<Transcript[]> {
	const pages = await Promise.all(
		batches(fileIds).map((batch) =>
			apiJson<{ items?: Transcript[] }>(
				`/api/transcripts?ids=${encodeURIComponent(batch.join(","))}`,
				{ fallbackError: "Không tải được bản chép lời" },
				fetcher,
			),
		),
	);
	return pages.flatMap((page) => page.items ?? []);
}

/** What to export: listed files, or every segment of an iPhone session (any length). */
export type ExportScope = { fileIds: readonly string[] } | { session: string };

/** Download path of the transcripts of `scope`, joined in recording order. */
export function transcriptExportPath(
	scope: ExportScope,
	format: "txt" | "srt",
	name: string,
): string {
	const query = new URLSearchParams(
		"session" in scope
			? { session: scope.session, format, name }
			: { ids: scope.fileIds.join(","), format, name },
	);
	return `/api/transcripts/export?${query.toString()}`;
}
