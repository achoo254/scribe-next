import { activeTranslator } from "../../i18n/active-translator";
/**
 * Streams MediaRecorder chunks of one recording session to the backend,
 * strictly one after another, each at the offset the backend last
 * reported. A chunk is retried (with backoff) at the same offset, so a
 * retry never appends twice. After the last retry fails the queue stops
 * sending; the recording is still finalized with what arrived. A 413
 * (storage or file size limit of the plan) is final at once: the backend
 * kept everything before that chunk, and `onFull` lets the recorder stop.
 */
import { ApiError, apiJson, errorMessage } from "../../lib/api";

export const CHUNK_RETRIES = 3;

const wait = (ms: number): Promise<void> =>
	new Promise((resolve) => setTimeout(resolve, ms));

export class RecordingChunkQueue {
	private written = 0;
	private failure: string | null = null;
	private chain: Promise<void> = Promise.resolve();
	private closed = false;
	private limitReached = false;

	constructor(
		private readonly sessionId: string,
		private readonly fetcher: typeof fetch = fetch,
		private readonly sleep: (ms: number) => Promise<void> = wait,
		private readonly onFull: (message: string) => void = () => undefined,
	) {}

	/** The backend refused a chunk because the plan's storage is full. */
	get full(): boolean {
		return this.limitReached;
	}

	/** Bytes the backend confirmed so far. */
	get offset(): number {
		return this.written;
	}

	/** Message of the chunk that could not be sent, if any. */
	get error(): string | null {
		return this.failure;
	}

	/** Queues a chunk behind the ones already queued. */
	enqueue(chunk: Blob): void {
		this.chain = this.chain.then(() => this.send(chunk));
	}

	/** Resolves once every queued chunk was sent or given up on. */
	drain(): Promise<void> {
		return this.chain;
	}

	/** Drops chunks queued after this point (session abandoned). */
	close(): void {
		this.closed = true;
	}

	private async send(chunk: Blob): Promise<void> {
		if (this.closed || this.failure !== null || chunk.size === 0) return;
		const body = await chunk.arrayBuffer();
		for (let attempt = 0; attempt < CHUNK_RETRIES; attempt += 1) {
			try {
				const reply = await apiJson<{ written: number }>(
					`/api/recordings/chunk?session=${this.sessionId}&offset=${this.written}`,
					{
						method: "POST",
						headers: { "Content-Type": "application/octet-stream" },
						body,
						fallbackError: activeTranslator().t("recorder.chunkFailed"),
					},
					this.fetcher,
				);
				this.written = reply.written;
				return;
			} catch (cause) {
				if (cause instanceof ApiError && cause.status === 413) {
					this.limitReached = true;
					this.failure = cause.message;
					this.onFull(cause.message);
					return;
				}
				if (attempt === CHUNK_RETRIES - 1) {
					this.failure = errorMessage(
						cause,
						activeTranslator().t("recorder.uploadFailed"),
					);
				} else {
					await this.sleep(300 * (attempt + 1));
				}
			}
		}
	}
}
