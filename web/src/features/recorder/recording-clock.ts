/**
 * Clock of a browser recording, from a monotonic start instead of counting
 * timer ticks: browsers throttle timers of hidden tabs (to once a minute or
 * less), so a tick count falls behind the audio being recorded.
 */
export class RecordingClock {
	private startedAt: number | null = null;
	private stoppedAt: number | null = null;

	constructor(private readonly now: () => number = () => performance.now()) {}

	start(): void {
		this.startedAt = this.now();
		this.stoppedAt = null;
	}

	/** Freezes the clock at the current time (the recording's length). */
	stop(): void {
		if (this.startedAt !== null && this.stoppedAt === null) {
			this.stoppedAt = this.now();
		}
	}

	/** Seconds since `start` (until `stop`); 0 before the first start. */
	seconds(): number {
		if (this.startedAt === null) return 0;
		const end = this.stoppedAt ?? this.now();
		return Math.max(0, (end - this.startedAt) / 1000);
	}
}
