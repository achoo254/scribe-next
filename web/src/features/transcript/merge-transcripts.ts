import type { Transcript, TranscriptStatus } from "./transcript-api";

/** One recording file in playback order (an iOS session has many). */
export interface TranscriptPart {
	fileId: string;
	durationS?: number;
}

export interface TranscriptLine {
	fileId: string;
	/** Seconds inside `fileId`: where the player seeks. */
	start: number;
	/** Seconds from the start of the whole recording: what the user reads. */
	at: number;
	text: string;
}

export interface MergedTranscript {
	/** Sentences of every finished file, in recording order. */
	lines: TranscriptLine[];
	/** Files with a finished transcript, out of `total`. */
	done: number;
	total: number;
	/** Files still waiting or being transcribed. */
	pending: number;
	failed: number;
	/** Overall 0–100, counting a finished file as 100. */
	progress: number;
	/** First error, to show; the same cause (bad key) usually hits every file. */
	error: string;
	/** Nothing has been asked yet for any of the files. */
	untouched: boolean;
}

const PENDING: ReadonlySet<TranscriptStatus> = new Set(["queued", "running"]);

/**
 * Joins the transcripts of the files of one recording: a later file's
 * sentences start after the duration of the files before it.
 */
export function mergeTranscripts(
	parts: readonly TranscriptPart[],
	transcripts: readonly Transcript[],
): MergedTranscript {
	const byId = new Map(transcripts.map((t) => [t.fileId, t]));
	const merged: MergedTranscript = {
		lines: [],
		done: 0,
		total: parts.length,
		pending: 0,
		failed: 0,
		progress: 0,
		error: "",
		untouched: true,
	};
	let offset = 0;
	let progressSum = 0;
	for (const part of parts) {
		const transcript = byId.get(part.fileId);
		if (transcript) {
			merged.untouched = false;
			if (transcript.status === "done") {
				merged.done += 1;
				progressSum += 100;
				for (const sentence of transcript.sentences) {
					merged.lines.push({
						fileId: part.fileId,
						start: sentence.start,
						at: offset + sentence.start,
						text: sentence.text,
					});
				}
			} else if (PENDING.has(transcript.status)) {
				merged.pending += 1;
				progressSum += transcript.progress;
			} else {
				merged.failed += 1;
				merged.error ||= transcript.error ?? "";
			}
		}
		offset += part.durationS ?? 0;
	}
	merged.progress =
		parts.length > 0 ? Math.floor(progressSum / parts.length) : 0;
	return merged;
}
