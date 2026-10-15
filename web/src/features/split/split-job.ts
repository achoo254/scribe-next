/**
 * Split job (Mode B, and "cắt bản ghi" in Mode A): create it, poll its
 * status until done or failed. Transient poll failures (network, 5xx) are
 * retried with a longer delay and only reported after several in a row.
 */
import { apiJson, errorMessage } from "../../lib/api";

export type JobCriterion = "minutes" | "megabytes" | "parts";
export type JobStatus = "queued" | "running" | "done" | "failed";

export interface JobSegment {
	name: string;
	fileId: string;
}

export interface SplitJobState {
	jobId: string | null;
	status: JobStatus | null;
	progress: number;
	segments: JobSegment[];
	error: string;
	creating: boolean;
}

export const IDLE_JOB: SplitJobState = {
	jobId: null,
	status: null,
	progress: 0,
	segments: [],
	error: "",
	creating: false,
};

interface JobStatusBody {
	status: JobStatus;
	progress: number;
	result?: { segments: JobSegment[] };
	error?: string;
}

export const POLL_INTERVAL_MS = 1500;
export const MAX_POLL_FAILS = 5;

export interface SplitRequest {
	inputFileId: string;
	criterion: JobCriterion;
	value: number;
	/** Segments become recordings (history) and the original is deleted. */
	recordingSplit?: boolean;
}

export function isJobBusy(status: JobStatus | null): boolean {
	return status === "queued" || status === "running";
}

export class SplitJob {
	private state: SplitJobState = IDLE_JOB;
	private timer: ReturnType<typeof setTimeout> | null = null;
	/** Bumped on reset/start so a late poll of an old job is ignored. */
	private generation = 0;

	constructor(
		private readonly onChange: (state: SplitJobState) => void,
		private readonly fetcher: typeof fetch = fetch,
	) {}

	get current(): SplitJobState {
		return this.state;
	}

	async start(request: SplitRequest): Promise<void> {
		this.stop();
		const generation = ++this.generation;
		this.set({ ...IDLE_JOB, creating: true });
		try {
			const { jobId } = await apiJson<{ jobId: string }>(
				"/api/jobs",
				{
					method: "POST",
					json: { recordingSplit: false, ...request },
					fallbackError: "Không thể tạo công việc cắt file",
				},
				this.fetcher,
			);
			if (generation !== this.generation) return;
			this.set({ ...IDLE_JOB, jobId, status: "queued" });
			void this.poll(jobId, generation, 0);
		} catch (cause) {
			if (generation !== this.generation) return;
			this.set({
				...IDLE_JOB,
				error: errorMessage(cause, "Không thể tạo công việc cắt file"),
			});
		}
	}

	/** Stops polling; the last known state stays. */
	stop(): void {
		if (this.timer !== null) clearTimeout(this.timer);
		this.timer = null;
	}

	/** Stops and forgets the job (a new file was chosen). */
	reset(): void {
		this.stop();
		this.generation += 1;
		this.set(IDLE_JOB);
	}

	private set(next: SplitJobState): void {
		this.state = next;
		this.onChange(next);
	}

	private schedule(
		jobId: string,
		generation: number,
		fails: number,
		delay: number,
	): void {
		this.timer = setTimeout(
			() => void this.poll(jobId, generation, fails),
			delay,
		);
	}

	private async poll(
		jobId: string,
		generation: number,
		fails: number,
	): Promise<void> {
		try {
			const body = await apiJson<JobStatusBody>(
				`/api/jobs/${encodeURIComponent(jobId)}`,
				{ fallbackError: "Không thể lấy tiến độ công việc" },
				this.fetcher,
			);
			if (generation !== this.generation) return;
			const base = {
				...this.state,
				status: body.status,
				progress: body.progress,
			};
			if (body.status === "done") {
				this.set({ ...base, segments: body.result?.segments ?? [] });
				return;
			}
			if (body.status === "failed") {
				this.set({ ...base, error: body.error ?? "Cắt file thất bại" });
				return;
			}
			this.set(base);
			this.schedule(jobId, generation, 0, POLL_INTERVAL_MS);
		} catch (cause) {
			if (generation !== this.generation) return;
			const next = fails + 1;
			if (next >= MAX_POLL_FAILS) {
				this.set({
					...this.state,
					error: errorMessage(cause, "Lỗi khi theo dõi tiến độ"),
				});
				return;
			}
			this.schedule(jobId, generation, next, POLL_INTERVAL_MS * 2);
		}
	}
}
