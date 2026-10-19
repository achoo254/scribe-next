import type { CurrentUser } from "../../src/auth/auth-api";
import type { RecordingItem, Usage } from "../../src/features/files/files-api";
import type {
	AskReply,
	SearchReply,
} from "../../src/features/search/search-api";
import type {
	TranscriptDetail,
	TranscriptMinutes,
	TranscriptSummary,
	TranscriptTranslation,
} from "../../src/features/transcript-docs/transcript-docs-api";
import { type ApiFixtures, apiFixtures } from "../support/api-stubs";

/*
 * Demo data of the App Store screenshots: one product meeting in
 * Vietnamese with three speakers, its minutes, bookmarks, an English
 * translation, a search and an answer. Invented people and email.
 */

const MB = 1024 * 1024;

export const DEMO_EMAIL = "lan.nguyen@example.com";

const USER: CurrentUser = {
	email: DEMO_EMAIL,
	canTranscribe: true,
	plan: {
		name: "plus",
		until: "2027-10-16T00:00:00+07:00",
		quotaBytes: 5 * 1024 * MB,
		maxFileBytes: 2 * 1024 * MB,
		retentionDays: 14,
	},
	transcription: {
		plan: "plus",
		planUntil: "2027-10-16T00:00:00+07:00",
		usedMinutes: 486,
		limitMinutes: 3000,
		resetsAt: "2026-11-01T00:00:00+07:00",
		capReached: false,
		paused: false,
		daily: null,
	},
};

const USAGE: Usage = {
	usageBytes: 1_288_490_189,
	quotaBytes: 5 * 1024 * MB,
	maxFileBytes: 2 * 1024 * MB,
	retentionDays: 14,
	plan: "plus",
};

/** The meeting: one recording (`f:` key) with speakers. */
export const MEETING_FILE = "65f0a1b2c3d4e5f607182a01";
export const MEETING_KEY = `f:${MEETING_FILE}`;
export const MEETING_PATH = `/transcripts/${encodeURIComponent(MEETING_KEY)}`;
const MEETING_TITLE = "Họp kế hoạch ra mắt ứng dụng";
const MEETING_AT = "2026-10-16T09:00:00+07:00";

const SENTENCES = [
	[
		2.1,
		6.8,
		"Chào cả nhà, hôm nay mình chốt kế hoạch ra mắt ứng dụng trên iPhone.",
		"s0",
	],
	[
		7.2,
		12.5,
		"Bản thử nghiệm đã chạy ổn trên mười máy trong tuần vừa rồi.",
		"s1",
	],
	[
		13.0,
		18.4,
		"Phần chép lời tiếng Việt rất sát, kể cả khi cả phòng cùng nói.",
		"s1",
	],
	[19.1, 23.6, "Còn phản hồi của khách hàng thì sao em?", "s0"],
	[
		24.0,
		30.2,
		"Khách thích nhất là biên bản tự động, có quyết định và việc cần làm kèm mốc thời gian.",
		"s2",
	],
	[
		30.8,
		35.9,
		"Họ muốn có thêm bản dịch tiếng Anh để gửi cho đối tác nước ngoài.",
		"s2",
	],
	[
		36.5,
		41.0,
		"Vậy mình chốt ra mắt vào thứ Hai, ngày hai mươi sáu tháng mười.",
		"s0",
	],
	[
		41.6,
		46.3,
		"Minh chuẩn bị bản phát hành, Hà gửi thông báo cho khách hàng trước thứ Sáu.",
		"s0",
	],
	[46.9, 49.5, "Dạ, em nhận việc.", "s2"],
] as const;

const ENGLISH = [
	"Hi everyone, today we finalise the plan to launch the iPhone app.",
	"The beta ran smoothly on ten phones last week.",
	"The Vietnamese transcription is very close, even when the whole room talks.",
	"And what about customer feedback?",
	"Customers love the automatic minutes most, with decisions and action items linked to the time.",
	"They want an English translation to send to partners abroad.",
	"So we launch on Monday, the twenty-sixth of October.",
	"Minh prepares the release, Hà notifies customers before Friday.",
	"Sure, I'll take it.",
];

const MINUTES: TranscriptMinutes = {
	model: "gpt-4.1-mini",
	createdAt: "2026-10-16T09:55:00+07:00",
	summary:
		"Nhóm chốt ra mắt ứng dụng trên iPhone vào thứ Hai 26/10. Bản thử nghiệm chạy ổn; khách hàng đánh giá cao biên bản tự động và muốn có bản dịch tiếng Anh.",
	decisions: [{ text: "Ra mắt ứng dụng vào thứ Hai 26/10.", at: 36.5 }],
	actions: [
		{ text: "Chuẩn bị bản phát hành", owner: "Minh", due: "thứ Sáu", at: 41.6 },
		{
			text: "Gửi thông báo ra mắt cho khách hàng",
			owner: "Hà",
			due: "thứ Sáu",
			at: 41.6,
		},
	],
	stale: false,
};

export const MEETING_DETAIL: TranscriptDetail = {
	key: MEETING_KEY,
	title: MEETING_TITLE,
	name: "Ghi âm 09:00",
	createdAt: MEETING_AT,
	segments: [
		{
			fileId: MEETING_FILE,
			index: 0,
			offsetS: 0,
			durationS: 1_845,
			hasVideo: false,
			status: "done",
			progress: 100,
			error: null,
			runId: "run-demo",
			edited: false,
			editedSentences: 0,
			sentences: SENTENCES.map(([start, end, text, speaker]) => ({
				start,
				end,
				text,
				speaker,
			})),
		},
	],
	missingIndexes: [],
	summary: MINUTES,
	bookmarks: [
		{ id: "65f0a1b2c3d4e5f607182b01", at: 24.0, label: "Phản hồi khách hàng" },
		{ id: "65f0a1b2c3d4e5f607182b02", at: 36.5, label: "Chốt ngày ra mắt" },
	],
	speakers: { names: { s0: "Chị Lan", s1: "Anh Minh", s2: "Hà" } },
};

const MEETING_SUMMARY: TranscriptSummary = {
	key: MEETING_KEY,
	title: MEETING_TITLE,
	name: "Ghi âm 09:00",
	createdAt: MEETING_AT,
	durationS: 1_845,
	segments: 1,
	done: 1,
	failed: 0,
	pending: 0,
	untranscribed: 0,
	preview: SENTENCES[0][2],
};

const OTHER_SUMMARIES: TranscriptSummary[] = [
	{
		key: "f:65f0a1b2c3d4e5f607182a02",
		title: "Phỏng vấn ứng viên thiết kế",
		name: null,
		createdAt: "2026-10-15T14:30:00+07:00",
		durationS: 2_710,
		segments: 1,
		done: 1,
		failed: 0,
		pending: 0,
		untranscribed: 0,
		preview: "Bạn kể về dự án gần nhất mà bạn tự hào nhất nhé.",
	},
	{
		key: "f:65f0a1b2c3d4e5f607182a03",
		title: "Gọi khách hàng Minh Phát",
		name: null,
		createdAt: "2026-10-14T10:05:00+07:00",
		durationS: 1_260,
		segments: 1,
		done: 1,
		failed: 0,
		pending: 0,
		untranscribed: 0,
		preview: "Bên em cần bản báo giá cho năm mươi người dùng.",
	},
];

export const MEETING_TRANSLATION: TranscriptTranslation = {
	lang: "en",
	status: "done",
	done: ENGLISH.length,
	total: ENGLISH.length,
	error: null,
	model: "gpt-4.1-mini",
	updatedAt: "2026-10-16T09:58:00+07:00",
	outdated: 0,
	missing: 0,
	segments: [
		{
			fileId: MEETING_FILE,
			sentences: ENGLISH.map((text) => ({ text, outdated: false })),
		},
	],
};

export const SEARCH_QUERY = "khách hàng";

const SEARCH: SearchReply = {
	results: [
		{
			key: MEETING_KEY,
			title: MEETING_TITLE,
			createdAt: MEETING_AT,
			fileId: MEETING_FILE,
			at: 19.1,
			text: SENTENCES[3][2],
			before: SENTENCES[2][2],
			after: SENTENCES[4][2],
		},
		{
			key: MEETING_KEY,
			title: MEETING_TITLE,
			createdAt: MEETING_AT,
			fileId: MEETING_FILE,
			at: 41.6,
			text: SENTENCES[7][2],
			before: SENTENCES[6][2],
			after: SENTENCES[8][2],
		},
		{
			key: "f:65f0a1b2c3d4e5f607182a03",
			title: "Gọi khách hàng Minh Phát",
			createdAt: "2026-10-14T10:05:00+07:00",
			fileId: "65f0a1b2c3d4e5f607182a03",
			at: 312,
			text: "Khách hàng hỏi có xuất được biên bản ra Word không.",
			before: null,
			after: null,
		},
	],
	total: 3,
};

export const QUESTION = "Khách hàng muốn gì thêm?";

const ANSWER: AskReply = {
	answer:
		"Khách hàng thích nhất biên bản tự động có quyết định và việc cần làm [1], và muốn có thêm bản dịch tiếng Anh để gửi đối tác nước ngoài [2].",
	citations: [
		{
			n: 1,
			key: MEETING_KEY,
			title: MEETING_TITLE,
			createdAt: MEETING_AT,
			at: 24.0,
			text: SENTENCES[4][2],
		},
		{
			n: 2,
			key: MEETING_KEY,
			title: MEETING_TITLE,
			createdAt: MEETING_AT,
			at: 30.8,
			text: SENTENCES[5][2],
		},
	],
};

const HISTORY: RecordingItem[] = [
	{
		id: MEETING_FILE,
		kind: "recording",
		name: "Ghi âm 09:00",
		durationS: 1_845,
		hasVideo: false,
		size: 29_884_416,
		createdAt: MEETING_AT,
		transcript: { status: "done" },
	},
	{
		id: "65f0a1b2c3d4e5f607182a02",
		kind: "upload",
		name: "phong-van-thiet-ke.m4a",
		durationS: 2_710,
		hasVideo: false,
		size: 43_909_120,
		createdAt: "2026-10-15T14:30:00+07:00",
		transcript: { status: "done" },
	},
];

export function demoFixtures(): ApiFixtures {
	return apiFixtures({
		user: USER,
		usage: USAGE,
		recordings: { retentionDays: 14, items: HISTORY },
		transcriptDocs: [MEETING_SUMMARY, ...OTHER_SUMMARIES],
		transcriptDetails: { [MEETING_KEY]: MEETING_DETAIL },
		transcripts: [],
		minutes: MINUTES,
		search: SEARCH,
		answer: ANSWER,
		translation: MEETING_TRANSLATION,
	});
}

/** Live transcript ("Chép trực tiếp") of the recording in progress. */
export const LIVE_SESSION = "7c1d2e3f-4a5b-4c6d-8e9f-0a1b2c3d4e5f";

export const LIVE_TRANSCRIPT = {
	sessionId: LIVE_SESSION,
	entries: [
		{
			id: "l1",
			atS: 1_498.2,
			text: "Tuần này mình ưu tiên hoàn thiện bản phát hành.",
		},
		{
			id: "l2",
			atS: 1_504.9,
			text: "Phần thông báo cho khách hàng em gửi bản nháp chiều nay.",
		},
		{
			id: "l3",
			atS: 1_511.6,
			text: "Nhớ thêm đường dẫn tải ứng dụng vào cuối thư nhé.",
		},
		{ id: "l4", atS: 1_517.3, text: "Dạ, em cập nhật luôn ạ." },
	],
	status: { connected: true },
};
