import type {
	TranscriptDetail,
	TranscriptSegment,
	TranscriptSummary,
	TranscriptTranslation,
} from "../../src/features/transcript-docs/transcript-docs-api";

/** An iPhone session of two transcribed one-minute segments. */
export const DOC_KEY = "s:a1b2c3d4-e5f6";
/** `/transcripts/:key` of `DOC_KEY` (the `:` is encoded). */
export const DOC_PATH = /\/transcripts\/s%3Aa1b2c3d4-e5f6$/;

function docSegment(index: number): TranscriptSegment {
	return {
		fileId: `f-iphone-${index}`,
		index,
		offsetS: index * 60,
		durationS: 60,
		hasVideo: false,
		status: "done",
		progress: 100,
		error: null,
		runId: `run-${index}`,
		edited: false,
		editedSentences: 0,
		sentences: [{ start: 0, end: 4, text: `Câu đầu của đoạn ${index + 1}.` }],
	};
}

export const DOC_SUMMARY: TranscriptSummary = {
	key: DOC_KEY,
	title: "Phỏng vấn khách hàng",
	name: null,
	createdAt: "2026-10-16T08:00:00+07:00",
	durationS: 120,
	segments: 2,
	done: 2,
	failed: 0,
	pending: 0,
	untranscribed: 0,
	preview: "Câu đầu của đoạn 1.",
};

export const DOC_DETAIL: TranscriptDetail = {
	key: DOC_KEY,
	title: "Phỏng vấn khách hàng",
	name: null,
	createdAt: "2026-10-16T08:00:00+07:00",
	segments: [docSegment(0), docSegment(1)],
	missingIndexes: [],
};

/** `DOC_KEY` translated into English; segment 1's sentence was edited since. */
export const DOC_TRANSLATION: TranscriptTranslation = {
	lang: "en",
	status: "done",
	done: 2,
	total: 2,
	error: null,
	model: "gpt-4.1-mini",
	updatedAt: "2026-10-16T09:59:00+07:00",
	outdated: 1,
	missing: 0,
	segments: [
		{
			fileId: "f-iphone-0",
			sentences: [{ text: "First sentence of segment 1.", outdated: true }],
		},
		{
			fileId: "f-iphone-1",
			sentences: [{ text: "First sentence of segment 2.", outdated: false }],
		},
	],
};

/** A one-file meeting transcribed with speakers: two people take turns. */
export const SPEAKERS_KEY = "f:65f0a1b2c3d4e5f60718293b";

export const SPEAKERS_DETAIL: TranscriptDetail = {
	key: SPEAKERS_KEY,
	title: "Họp giao việc",
	name: "Ghi âm 09:00",
	createdAt: "2026-10-16T09:00:00+07:00",
	segments: [
		{
			...docSegment(0),
			fileId: "65f0a1b2c3d4e5f60718293b",
			durationS: 45,
			sentences: [
				{
					start: 0.4,
					end: 3.1,
					text: "Chào cả nhà, mình bắt đầu nhé.",
					speaker: "s0",
				},
				{
					start: 3.6,
					end: 7.9,
					text: "Tuần này em lo phần báo giá cho khách.",
					speaker: "s0",
				},
				{
					start: 8.5,
					end: 11.2,
					text: "Dạ, thứ Sáu em gửi bản đầu.",
					speaker: "s1",
				},
				{ start: 11.8, end: 14.0, text: "Ổn, chốt vậy nhé.", speaker: "s0" },
			],
		},
	],
	missingIndexes: [],
	speakers: { names: { s0: "Người 1", s1: "Người 2" } },
};
