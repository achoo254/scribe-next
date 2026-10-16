import { activeTranslator } from "../../i18n/active-translator";
import { apiFetch, apiJson } from "../../lib/api";
import type { TranscriptStatus } from "../transcript/transcript-api";

/** A browser recording (Mode A), listed in "Lịch sử ghi". */
export interface RecordingItem {
	id: string;
	name: string;
	durationS?: number;
	hasVideo: boolean;
	size: number;
	createdAt?: string;
	/** Set once the recording was sent for transcription. */
	transcript?: { status: TranscriptStatus } | null;
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
}

export interface Usage {
	usageBytes: number;
	quotaBytes: number;
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

/** Deletes a recording, upload or segment (one route for every kind). */
export async function deleteFile(
	id: string,
	fetcher: typeof fetch = fetch,
): Promise<void> {
	await apiFetch(
		`/api/files/${encodeURIComponent(id)}`,
		{
			method: "DELETE",
			fallbackError: activeTranslator().t("errors.files.delete"),
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
