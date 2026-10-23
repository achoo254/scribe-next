import { activeTranslator } from "../../i18n/active-translator";
import { apiFetch, apiJson } from "../../lib/api";
import type { TranscriptStatus } from "../transcript/transcript-api";

/**
 * An item of "Lịch sử ghi": a recording (browser or iPhone segment) or an
 * audio/video file uploaded with "Tải tệp lên" on the record page; both can
 * be transcribed. "Cắt file" uploads are not listed.
 */
export interface RecordingItem {
	id: string;
	name: string;
	/** Missing in older replies: a recording. */
	kind?: "recording" | "upload";
	durationS?: number;
	hasVideo: boolean;
	size: number;
	createdAt?: string;
	/** When the backend sweeps this file (set by the plan at creation). */
	expiresAt?: string;
	/** Set once the recording was sent for transcription. */
	transcript?: { status: TranscriptStatus } | null;
	/**
	 * Name the user gave the recording (every segment of an iOS session
	 * carries the session's); null or missing = the default name.
	 */
	title?: string | null;
}

export interface RecordingList {
	items: RecordingItem[];
	/** Days before the backend sweeps a file; 0 = unknown (hide the notice). */
	retentionDays: number;
}

/** A file that is not a recording: a split source or a split segment. */
export interface ManagedFile {
	id: string;
	name: string;
	kind: "upload" | "output";
	size: number;
	jobId?: string;
	createdAt?: string;
	expiresAt?: string;
}

export interface Usage {
	usageBytes: number;
	/** Storage of the user's plan (free 200 MB, plus 5 GB). */
	quotaBytes: number;
	/** Largest single file of the plan. */
	maxFileBytes?: number;
	/** Days a new file is kept. */
	retentionDays?: number;
	plan?: "free" | "plus";
}

/** What "Xoá tất cả" did; `skipped` files belong to a recording still in progress. */
export interface DeleteAllResult {
	deleted: number;
	skipped: number;
	failed: number;
	bytes: number;
}

export async function listRecordings(
	fetcher: typeof fetch = fetch,
): Promise<RecordingList> {
	const body = await apiJson<{
		items?: RecordingItem[];
		retentionDays?: number;
	}>(
		"/api/files",
		{ fallbackError: activeTranslator().t("errors.files.history") },
		fetcher,
	);
	return {
		items: body.items ?? [],
		retentionDays:
			typeof body.retentionDays === "number" ? body.retentionDays : 0,
	};
}

export async function listManagedFiles(
	fetcher: typeof fetch = fetch,
): Promise<ManagedFile[]> {
	const body = await apiJson<{ items?: ManagedFile[] }>(
		"/api/files/manage",
		{ fallbackError: activeTranslator().t("errors.files.list") },
		fetcher,
	);
	return body.items ?? [];
}

/**
 * Deletes a recording, upload or segment (one route for every kind).
 * `keepalive` lets the browser finish the request while the page unloads.
 */
export async function deleteFile(
	id: string,
	fetcher: typeof fetch = fetch,
	{ keepalive = false }: { keepalive?: boolean } = {},
): Promise<void> {
	await apiFetch(
		`/api/files/${encodeURIComponent(id)}`,
		{
			method: "DELETE",
			keepalive,
			fallbackError: activeTranslator().t("errors.files.delete"),
		},
		fetcher,
	);
}

/**
 * Deletes what "Lịch sử ghi" lists: recordings and files uploaded on the
 * record page ("Cắt file" files stay). Spares recordings still in progress
 * (`skipped`).
 */
export function deleteAllFiles(
	fetcher: typeof fetch = fetch,
): Promise<DeleteAllResult> {
	return apiJson<DeleteAllResult>(
		"/api/files",
		{
			method: "DELETE",
			fallbackError: activeTranslator().t("errors.files.deleteAll"),
		},
		fetcher,
	);
}

export function fetchUsage(fetcher: typeof fetch = fetch): Promise<Usage> {
	return apiJson<Usage>(
		"/api/me/usage",
		{ fallbackError: activeTranslator().t("errors.files.usage") },
		fetcher,
	);
}
