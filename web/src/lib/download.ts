import { apiUrl } from "./api";

/**
 * Starts a download of an `/api/...` URL. The backend answers with
 * `Content-Disposition: attachment`, so navigating keeps the page and the
 * browser saves the file under the name the backend chose. A native shell
 * (iOS plan) replaces this with its own file handling.
 */
export function download(path: string): void {
	window.location.href = apiUrl(path);
}

/** Stream URL of a stored file (HTTP Range, so players can seek). */
export function mediaUrl(fileId: string): string {
	return apiUrl(`/api/files/${encodeURIComponent(fileId)}`);
}

/** Paths of the download endpoints, in one place. */
export const downloadPaths = {
	file: (fileId: string) =>
		`/api/files/${encodeURIComponent(fileId)}?download=1`,
	allRecordings: () => "/api/files/archive",
	recordingsBetween: (from: number, to: number) =>
		`/api/files/archive?from=${from}&to=${to}`,
	jobResult: (jobId: string) =>
		`/api/jobs/${encodeURIComponent(jobId)}/download`,
} as const;
