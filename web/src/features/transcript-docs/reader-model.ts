import { activeTranslator } from "../../i18n/active-translator";
import { sessionTitle } from "../files/recording-groups";
import type {
	Bookmark,
	SegmentRunStatus,
	TranscriptDetail,
	TranscriptSegment,
	TranscriptSummary,
} from "./transcript-docs-api";

/** Speaker ids of the names map, in order: `s0`, `s1`, … `s10`. */
export function speakerIds(names: Readonly<Record<string, string>>): string[] {
	const number = (id: string) => Number.parseInt(id.slice(1), 10) || 0;
	return Object.keys(names).sort((a, b) => number(a) - number(b));
}

/**
 * Speaker heard just before each segment: the speaker of the last sentence
 * of the closest earlier segment that has sentences (null at the start).
 * A segment shows a speaker label on its first sentence only when it differs.
 */
export function speakersBefore(
	segments: readonly TranscriptSegment[],
): Map<string, string | null> {
	const out = new Map<string, string | null>();
	let last: string | null = null;
	for (const segment of segments) {
		out.set(segment.fileId, last);
		const done = segment.status === "done" ? segment.sentences : [];
		const tail = done[done.length - 1];
		if (tail) last = tail.speaker ?? null;
	}
	return out;
}

/** Seconds before the end of a segment when the next one starts loading. */
export const PRELOAD_BEFORE_END_S = 10;

/**
 * Index of the sentence being heard in `segment` at `time` (seconds inside
 * the segment's file): the last one that started; -1 before the first.
 */
export function activeSentence(
	segment: TranscriptSegment,
	time: number,
): number {
	let active = -1;
	segment.sentences.forEach((sentence, index) => {
		if (sentence.start <= time) active = index;
	});
	return active;
}

/**
 * Sentence a moment of the segment belongs to (bookmarks sit beside it):
 * the one being said then, or the first when the moment comes before it;
 * -1 when the segment has no sentences.
 */
export function sentenceAt(segment: TranscriptSegment, time: number): number {
	if (segment.sentences.length === 0) return -1;
	return Math.max(0, activeSentence(segment, time));
}

/** Where a session time (seconds from its start) is: segment and time in its file. */
export interface SessionPlace {
	segment: TranscriptSegment;
	/** Seconds inside the segment's own file. */
	seconds: number;
}

/**
 * The segment holding `at` (seconds from the session start) and the time
 * inside it. A moment in a missing segment or past the end goes to the
 * segment before it (clamped to its end); before the first, to its start.
 */
export function placeOf(
	segments: readonly TranscriptSegment[],
	at: number,
): SessionPlace | null {
	const first = segments[0];
	if (!first) return null;
	let segment = first;
	for (const candidate of segments) {
		if (candidate.offsetS <= at) segment = candidate;
	}
	const inside = Math.max(0, at - segment.offsetS);
	return {
		segment,
		seconds:
			segment.durationS > 0 ? Math.min(inside, segment.durationS) : inside,
	};
}

/** Bookmarks of each segment (by file id), in time order. */
export function bookmarksBySegment(
	segments: readonly TranscriptSegment[],
	bookmarks: readonly Bookmark[],
): Map<string, Bookmark[]> {
	const out = new Map<string, Bookmark[]>();
	for (const bookmark of [...bookmarks].sort((a, b) => a.at - b.at)) {
		const place = placeOf(segments, bookmark.at);
		if (!place) continue;
		const list = out.get(place.segment.fileId) ?? [];
		list.push(bookmark);
		out.set(place.segment.fileId, list);
	}
	return out;
}

/** Segment that plays after `fileId`; null after the last one. */
export function nextSegment(
	segments: readonly TranscriptSegment[],
	fileId: string,
): TranscriptSegment | null {
	const at = segments.findIndex((segment) => segment.fileId === fileId);
	return at >= 0 ? (segments[at + 1] ?? null) : null;
}

/** Time to start loading the next segment so playback carries on without a gap. */
export function shouldPreload(time: number, durationS: number): boolean {
	return durationS > 0 && durationS - time <= PRELOAD_BEFORE_END_S;
}

/**
 * Missing segment numbers that belong right before each segment (shown as
 * "Thiếu đoạn N" where the gap is), keyed by the segment's file id.
 */
export function gapsBefore(
	detail: Pick<TranscriptDetail, "segments" | "missingIndexes">,
): Map<string, number[]> {
	const gaps = new Map<string, number[]>();
	let previous = -1;
	for (const segment of detail.segments) {
		const missing = detail.missingIndexes.filter(
			(index) => index > previous && index < segment.index,
		);
		if (missing.length > 0) gaps.set(segment.fileId, missing);
		previous = segment.index;
	}
	return gaps;
}

/** What transcribing these segments again would throw away. */
export function editedCounts(segments: readonly TranscriptSegment[]): {
	segments: number;
	sentences: number;
} {
	const edited = segments.filter((segment) => segment.edited);
	return {
		segments: edited.length,
		sentences: edited.reduce((sum, s) => sum + s.editedSentences, 0),
	};
}

/** Segments the user can still send: never transcribed, or failed. */
export function untranscribed(
	segments: readonly TranscriptSegment[],
): TranscriptSegment[] {
	return segments.filter((s) => s.status === "none" || s.status === "failed");
}

export function isPending(status: TranscriptSegment["status"]): boolean {
	return status === "queued" || status === "running";
}

/**
 * The polled statuses differ from the loaded transcript in a way that
 * changes its sentences (a segment finished, failed, or was queued again):
 * time to reload the full transcript. Progress alone does not count.
 */
export function needsReload(
	segments: readonly TranscriptSegment[],
	statuses: readonly SegmentRunStatus[],
): boolean {
	const loaded = new Map(segments.map((s) => [s.fileId, s]));
	if (statuses.length !== segments.length) return true;
	return statuses.some((status) => {
		const segment = loaded.get(status.fileId);
		return (
			!segment ||
			segment.status !== status.status ||
			segment.runId !== status.runId
		);
	});
}

/** Overall progress 0–100 of the segments sent for transcription. */
export function overallProgress(
	segments: readonly TranscriptSegment[],
	statuses: readonly SegmentRunStatus[] = [],
): number {
	const live = new Map(statuses.map((s) => [s.fileId, s]));
	const sent = segments.filter((s) => s.status !== "none");
	if (sent.length === 0) return 0;
	const sum = sent.reduce((total, segment) => {
		const status = live.get(segment.fileId) ?? segment;
		return total + (status.status === "done" ? 100 : status.progress);
	}, 0);
	return Math.floor(sum / sent.length);
}

/**
 * Name to show: the user's title, else "Bản ghi trên iPhone 09:41 16/10/2026"
 * for a session or the file name of a browser recording.
 */
export function transcriptTitle(
	doc: Pick<TranscriptSummary, "key" | "title" | "name" | "createdAt">,
): string {
	if (doc.title) return doc.title;
	if (doc.key.startsWith("s:"))
		return sessionTitle(doc.createdAt, { withDate: true });
	return doc.name || activeTranslator().t("transcript.defaultName");
}

export type SummaryTone = "info" | "danger" | "warning" | "success";

/** One-line state of a transcript in the list. */
export function summaryStatus(item: TranscriptSummary): {
	label: string;
	tone: SummaryTone;
} {
	const { t } = activeTranslator();
	const sent = item.segments - item.untranscribed;
	if (item.pending > 0) {
		return {
			label: t("transcript.status.running", { done: item.done, sent }),
			tone: "info",
		};
	}
	if (item.failed > 0) {
		return {
			label: t("transcript.status.failed", { count: item.failed }),
			tone: "danger",
		};
	}
	if (item.untranscribed > 0) {
		return {
			label: t("transcript.status.untranscribed", {
				count: item.untranscribed,
			}),
			tone: "warning",
		};
	}
	return { label: t("transcript.status.done"), tone: "success" };
}
