import { activeTranslator } from "../../i18n/active-translator";
/**
 * Uploads one file in 5 MB chunks that the backend appends straight to
 * disk. Flow: init (backend creates a session) → status (bytes already
 * written) → POST each chunk at that offset → finalize (file id, duration).
 *
 * The session survives a failure: calling `upload()` again with the same
 * File resumes from the offset the backend reports instead of starting over.
 */
import { apiJson } from "../../lib/api";

export const UPLOAD_CHUNK_SIZE = 5 * 1024 * 1024;

export interface UploadResult {
	fileId: string;
	name: string;
	durationS: number;
}

/** Whole percent of `file` already on the server. */
export function percentOf(written: number, size: number): number {
	return size > 0 ? Math.floor((written / size) * 100) : 100;
}

export class ChunkedUpload {
	private sessionId: string | null = null;
	private sessionFile: File | null = null;

	constructor(
		private readonly fetcher: typeof fetch = fetch,
		private readonly chunkSize: number = UPLOAD_CHUNK_SIZE,
	) {}

	/**
	 * Sends `file` and returns the stored file. Throws an `ApiError` with
	 * the backend's (Vietnamese) message; the session is kept for a resume.
	 */
	async upload(
		file: File,
		onProgress: (percent: number) => void = () => undefined,
	): Promise<UploadResult> {
		const session = await this.sessionFor(file);

		const status = await apiJson<{ written: number }>(
			`/api/upload/status?session=${session}`,
			{ fallbackError: activeTranslator().t("errors.upload.status") },
			this.fetcher,
		);
		let offset = status.written;
		onProgress(percentOf(offset, file.size));

		while (offset < file.size) {
			const chunk = file.slice(
				offset,
				Math.min(offset + this.chunkSize, file.size),
			);
			const written = await apiJson<{ written: number }>(
				`/api/upload/chunk?session=${session}&offset=${offset}`,
				{
					method: "POST",
					headers: { "Content-Type": "application/octet-stream" },
					body: chunk,
					fallbackError: activeTranslator().t("errors.upload.chunk"),
				},
				this.fetcher,
			);
			if (written.written <= offset) {
				throw new Error(activeTranslator().t("errors.upload.noProgress"));
			}
			offset = written.written;
			onProgress(percentOf(offset, file.size));
		}

		const result = await apiJson<UploadResult>(
			"/api/upload/finalize",
			{
				method: "POST",
				json: { session },
				fallbackError: activeTranslator().t("errors.upload.complete"),
			},
			this.fetcher,
		);
		onProgress(100);
		// Done: the next file gets a new session.
		this.reset();
		return result;
	}

	/** Forgets the session (a new file, or the user starts over). */
	reset(): void {
		this.sessionId = null;
		this.sessionFile = null;
	}

	private async sessionFor(file: File): Promise<string> {
		if (this.sessionId !== null && this.sessionFile === file) {
			return this.sessionId;
		}
		const init = await apiJson<{ sessionId: string }>(
			"/api/upload/init",
			{
				method: "POST",
				json: { name: file.name, size: file.size },
				fallbackError: activeTranslator().t("errors.upload.init"),
			},
			this.fetcher,
		);
		this.sessionId = init.sessionId;
		this.sessionFile = file;
		return init.sessionId;
	}
}
