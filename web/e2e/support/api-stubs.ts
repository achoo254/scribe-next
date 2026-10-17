import type { Page, Route } from "@playwright/test";
import type { CurrentUser } from "../../src/auth/auth-api";
import type {
	ChatLinkStart,
	ChatLinkStatus,
} from "../../src/features/chat-link/chat-link-api";
import type {
	DeleteAllResult,
	ManagedFile,
	RecordingItem,
	RecordingList,
	Usage,
} from "../../src/features/files/files-api";
import type {
	AskReply,
	SearchReply,
} from "../../src/features/search/search-api";
import type { JobSegment } from "../../src/features/split/split-job";
import type {
	Bookmark,
	TranscriptDetail,
	TranscriptMinutes,
	TranscriptSummary,
} from "../../src/features/transcript-docs/transcript-docs-api";
import type { Transcript } from "../../src/features/transcript/transcript-api";
import { E2E_ORIGIN } from "./e2e-origin";
import { E2E_STUB_MARKER } from "./stub-markers";

/**
 * What the stubbed backend answers. Shapes come from the app's own types,
 * so a change of the API contract fails `pnpm run typecheck` here too.
 */
export interface ApiFixtures {
	/** null: no session (`/api/me` answers 401). */
	user: CurrentUser | null;
	usage: Usage;
	recordings: RecordingList;
	managedFiles: ManagedFile[];
	transcriptDocs: TranscriptSummary[];
	/** `GET /api/transcript-docs/:key`, by decoded key. */
	transcriptDetails: Record<string, TranscriptDetail>;
	transcripts: Transcript[];
	/** Result of every split job (`GET /api/jobs/:id` answers "done"). */
	splitSegments: JobSegment[];
	/** `GET /api/chat/link` until the spec links or unlinks. */
	chatLinked: boolean;
	/** What `POST /api/transcript-docs/:key/summary` makes. */
	minutes: TranscriptMinutes;
	/** What `GET /api/search` finds, whatever the query. */
	search: SearchReply;
	/** What `POST /api/ask` answers, whatever the question. */
	answer: AskReply;
	/** Replies that win over the stub's own, by `METHOD /path` (e.g. an error). */
	replies: Record<string, StubReply>;
}

export interface StubReply {
	status: number;
	body: unknown;
}

/** The deep link `POST /api/chat/link` answers. */
export const CHAT_LINK: ChatLinkStart = {
	url: "https://chat.hoandat.com/b/bot_scribe_bot?start=e2e-token",
	expiresAt: "2026-10-16T10:10:00+07:00",
};

/** `linkedAt` of a linked account. */
const LINKED_AT = "2026-10-16T09:55:00+07:00";
/** `chatName` of a linked account: who confirmed the link in the chat. */
export const LINKED_CHAT_NAME = "An Nguyễn";

export const STUB_MINUTES: TranscriptMinutes = {
	model: "gpt-4.1-mini",
	createdAt: "2026-10-16T09:50:00+07:00",
	summary:
		"Khách hàng muốn bản dùng thử trong tháng này; nhóm sẽ gửi báo giá và lịch triển khai.",
	decisions: [{ text: "Bắt đầu dùng thử từ thứ Hai tuần sau.", at: 62 }],
	actions: [
		{
			text: "Gửi báo giá gói doanh nghiệp",
			owner: "Lan",
			due: "thứ Sáu",
			at: 8,
		},
		{ text: "Chuẩn bị tài khoản dùng thử", owner: null, due: null, at: 95 },
	],
	stale: false,
};

/** Every date in the fixtures is this day (Asia/Saigon); specs freeze the clock to it. */
export const FIXED_NOW = new Date("2026-10-16T10:00:00+07:00");

const MB = 1024 * 1024;

export const SIGNED_IN_USER: CurrentUser = {
	email: "minh.anh@example.com",
	canTranscribe: true,
	plan: {
		name: "free",
		until: null,
		quotaBytes: 200 * MB,
		maxFileBytes: 200 * MB,
		retentionDays: 7,
	},
	transcription: {
		plan: "free",
		planUntil: null,
		usedMinutes: 42,
		limitMinutes: 120,
		resetsAt: "2026-11-01T00:00:00+07:00",
		capReached: false,
		paused: false,
		daily: { usedMinutes: 12, limitMinutes: 30 },
	},
};

/** What a file uploaded on the record page becomes in "Lịch sử ghi". */
export const STUB_UPLOADED: RecordingItem = {
	id: "f-e2e-upload",
	kind: "upload",
	name: "phong-van.m4a",
	durationS: 300,
	hasVideo: false,
	size: 4_718_592,
	createdAt: "2026-10-16T09:58:00+07:00",
	transcript: null,
};

/** The sentence the stubbed transcription gives every newly queued file. */
export const STUB_SENTENCE = "Câu chép lời từ tệp tải lên.";

const IPHONE_SESSION = "a1b2c3d4-e5f6";

export function apiFixtures(overrides: Partial<ApiFixtures> = {}): ApiFixtures {
	return {
		user: SIGNED_IN_USER,
		usage: {
			usageBytes: 48_234_496,
			quotaBytes: 200 * MB,
			maxFileBytes: 200 * MB,
			retentionDays: 7,
			plan: "free",
		},
		recordings: {
			retentionDays: 7,
			items: [
				{
					id: "f-browser-1",
					name: "Họp nhóm sáng thứ Năm.webm",
					durationS: 1_845,
					hasVideo: false,
					size: 14_680_064,
					createdAt: "2026-10-16T09:12:00+07:00",
					transcript: { status: "done" },
				},
				{
					id: "f-iphone-1",
					name: `${IPHONE_SESSION}#1`,
					durationS: 60,
					hasVideo: false,
					size: 983_040,
					createdAt: "2026-10-16T08:01:00+07:00",
					transcript: null,
				},
				{
					id: "f-iphone-0",
					name: `${IPHONE_SESSION}#0`,
					durationS: 60,
					hasVideo: false,
					size: 983_040,
					createdAt: "2026-10-16T08:00:00+07:00",
					transcript: null,
				},
			],
		},
		managedFiles: [],
		transcriptDocs: [],
		transcriptDetails: {},
		transcripts: [
			{
				fileId: "f-browser-1",
				status: "done",
				progress: 100,
				error: null,
				sentences: [
					{ start: 0, end: 4.2, text: "Chào mọi người, mình bắt đầu nhé." },
				],
			},
		],
		splitSegments: [
			{ name: "phong-van-phan-1.m4a", fileId: "f-seg-1" },
			{ name: "phong-van-phan-2.m4a", fileId: "f-seg-2" },
		],
		chatLinked: false,
		minutes: STUB_MINUTES,
		search: { results: [], total: 0 },
		answer: { answer: "Không tìm thấy trong bản ghi.", citations: [] },
		replies: {},
		...overrides,
	};
}

/**
 * `UPLOAD_CHUNK_SIZE` of src/features/upload/chunked-upload.ts. Not
 * imported: that module pulls in `import.meta.env`, which Node lacks.
 */
const UPLOAD_CHUNK_SIZE = 5 * 1024 * 1024;

/** Ids the stubbed backend hands out for new sessions and jobs. */
export const STUB_IDS = {
	recordingSession: "e2e-recording",
	/** File id `POST /api/recordings/init` keeps for the browser recording. */
	recordingFile: "6710e2e0a1b2c3d4e5f60718",
	uploadSession: "e2e-upload",
	job: "e2e-job",
} as const;

/** One `POST /api/recordings/chunk` of a browser recording. */
export interface RecordingChunk {
	session: string;
	offset: number;
	size: number;
}

export interface ApiStub {
	/** `METHOD /path` of every `/api` request no stub answers. */
	readonly unexpected: string[];
	/** `METHOD /path` of every `/api` request, in order. */
	readonly requests: string[];
	/** Chunks of browser recordings, in the order they arrived. */
	readonly recordingChunks: RecordingChunk[];
	/** `POST /api/transcript-docs/:key/bookmarks`, by decoded key. */
	readonly bookmarks: Array<{ key: string; at: number; id: string | null }>;
	/** `q` of every `GET /api/search`, in order. */
	readonly searches: string[];
	/** Body of every `POST /api/ask`, in order. */
	readonly questions: Array<{ question: string | null; key: string | null }>;
	/** `fileIds` of every `POST /api/transcripts`, in order. */
	readonly transcribed: string[][];
	/** Path of every `DELETE /api/files…`, in order. */
	readonly deletes: string[];
	/** Stands in for the user sending `/start` to the bot on chat.hoandat.com. */
	linkChat: () => void;
}

/** Number of `METHOD /path` calls the stub answered. */
export function countOf(stub: ApiStub, call: string): number {
	return stub.requests.filter((request) => request === call).length;
}

/** `/api/transcript-docs/<key>/<summary|share|bookmarks>[/<id>]`. */
const DOC_ACTION =
	/^\/api\/transcript-docs\/([^/]+)\/(summary|share|bookmarks)(?:\/([^/]+))?$/;

/** A number field of a JSON body, or NaN. */
function stringField(body: unknown, name: string): string | null {
	if (typeof body !== "object" || body === null) return null;
	const value: unknown = Reflect.get(body, name);
	return typeof value === "string" ? value : null;
}

function numberField(body: unknown, name: string): number {
	if (typeof body !== "object" || body === null) return Number.NaN;
	const value: unknown = Reflect.get(body, name);
	return typeof value === "number" ? value : Number.NaN;
}

function json(route: Route, status: number, body: unknown): Promise<void> {
	return route.fulfill({
		status,
		contentType: "application/json",
		body: JSON.stringify(body),
	});
}

/**
 * Answers `/api/*` from `fixtures` and refuses every other origin, so a
 * spec never reaches the dev backend (8787) or production, even through
 * the native shell's absolute API origin.
 */
export async function stubApi(
	page: Page,
	fixtures: ApiFixtures = apiFixtures(),
): Promise<ApiStub> {
	let chatLinked = fixtures.chatLinked;
	const stub: ApiStub = {
		unexpected: [],
		requests: [],
		recordingChunks: [],
		bookmarks: [],
		searches: [],
		questions: [],
		transcribed: [],
		deletes: [],
		linkChat: () => {
			chatLinked = true;
		},
	};
	// Bytes the stubbed backend holds per upload session ("Cắt file"), and
	// the size announced at init.
	const uploaded = new Map<string, number>();
	let uploadSize = 0;
	// "Lịch sử ghi" and transcripts change as the spec uploads, transcribes
	// and deletes.
	let historyItems: RecordingItem[] = [...fixtures.recordings.items];
	const transcripts = [...fixtures.transcripts];

	await page.route(
		(url) => url.origin !== E2E_ORIGIN,
		(route) => route.abort("blockedbyclient"),
	);

	await page.route(
		(url) => url.origin === E2E_ORIGIN && url.pathname.startsWith("/api/"),
		async (route) => {
			const request = route.request();
			const url = new URL(request.url());
			const call = `${request.method()} ${url.pathname}`;
			stub.requests.push(call);
			const session = url.searchParams.get("session") ?? "";
			const offset = Number(url.searchParams.get("offset") ?? "0");
			const bodySize = request.postDataBuffer()?.length ?? 0;
			const override = fixtures.replies[call];
			if (override) return json(route, override.status, override.body);
			const docAction = DOC_ACTION.exec(url.pathname);
			if (docAction) {
				const [, rawKey = "", action, id] = docAction;
				const key = decodeURIComponent(rawKey);
				if (request.method() === "POST" && action === "summary") {
					return json(route, 200, fixtures.minutes);
				}
				if (request.method() === "POST" && action === "share") {
					return chatLinked
						? json(route, 200, { ok: true })
						: json(route, 409, {
								error: "Chưa liên kết chat.hoandat.com.",
								code: "CHAT_NOT_LINKED",
							});
				}
				if (request.method() === "POST" && action === "bookmarks" && !id) {
					const body: unknown = request.postDataJSON();
					const at = numberField(body, "at");
					const clientId = stringField(body, "id");
					stub.bookmarks.push({ key, at, id: clientId });
					const saved: Bookmark = {
						id: clientId ?? `b-${stub.bookmarks.length}`,
						at,
						label: null,
					};
					return json(route, 200, saved);
				}
				if (request.method() === "DELETE" && action === "bookmarks" && id) {
					return json(route, 200, { ok: true });
				}
			}
			if (
				request.method() === "GET" &&
				url.pathname.startsWith("/api/transcript-docs/")
			) {
				const key = decodeURIComponent(
					url.pathname.slice("/api/transcript-docs/".length),
				);
				const detail = fixtures.transcriptDetails[key];
				if (detail) return json(route, 200, detail);
			}
			switch (call) {
				case "GET /api/me":
					return fixtures.user
						? json(route, 200, fixtures.user)
						: json(route, 401, { error: "Chưa đăng nhập" });
				case "GET /api/me/usage":
					return json(route, 200, fixtures.usage);
				case "GET /api/search":
					stub.searches.push(url.searchParams.get("q") ?? "");
					return json(route, 200, fixtures.search);
				case "POST /api/ask": {
					const body: unknown = request.postDataJSON();
					stub.questions.push({
						question: stringField(body, "question"),
						key: stringField(body, "key"),
					});
					return json(route, 200, fixtures.answer);
				}
				case "GET /api/files":
					return json(route, 200, {
						...fixtures.recordings,
						items: historyItems,
					});
				case "DELETE /api/files": {
					stub.deletes.push(url.pathname);
					const result: DeleteAllResult = {
						deleted: historyItems.length,
						skipped: 0,
						failed: 0,
						bytes: historyItems.reduce((sum, item) => sum + item.size, 0),
					};
					historyItems = [];
					return json(route, 200, result);
				}
				case "POST /api/transcripts": {
					const body: unknown = request.postDataJSON();
					const ids: unknown =
						typeof body === "object" && body !== null
							? Reflect.get(body, "fileIds")
							: null;
					const fileIds = Array.isArray(ids)
						? ids.filter((id): id is string => typeof id === "string")
						: [];
					stub.transcribed.push(fileIds);
					for (const fileId of fileIds) {
						if (transcripts.some((t) => t.fileId === fileId)) continue;
						transcripts.push({
							fileId,
							status: "done",
							progress: 100,
							error: null,
							sentences: [{ start: 0, end: 3, text: STUB_SENTENCE }],
						});
					}
					historyItems = historyItems.map((item) =>
						fileIds.includes(item.id)
							? { ...item, transcript: { status: "done" } }
							: item,
					);
					return json(route, 200, { queued: fileIds.length, skipped: 0 });
				}
				case "GET /api/files/manage":
					return json(route, 200, { items: fixtures.managedFiles });
				case "GET /api/transcript-docs":
					return json(route, 200, { items: fixtures.transcriptDocs });
				case "GET /api/transcripts": {
					const ids = new Set(
						(url.searchParams.get("ids") ?? "").split(",").filter(Boolean),
					);
					return json(route, 200, {
						items: transcripts.filter((t) => ids.has(t.fileId)),
					});
				}
				case "GET /api/auth/login":
					// Stands in for the redirect to hoandat-auth.
					return route.fulfill({
						status: 200,
						contentType: "text/html; charset=utf-8",
						body: `<!doctype html><html lang="vi"><title>${E2E_STUB_MARKER}</title><p>Trang đăng nhập hoandat (giả lập)</p></html>`,
					});
				case "GET /api/chat/link": {
					const status: ChatLinkStatus = {
						linked: chatLinked,
						linkedAt: chatLinked ? LINKED_AT : null,
						chatName: chatLinked ? LINKED_CHAT_NAME : null,
						enabled: true,
					};
					return json(route, 200, status);
				}
				case "POST /api/chat/link":
					return json(route, 200, CHAT_LINK);
				case "DELETE /api/chat/link":
					chatLinked = false;
					return json(route, 200, { linked: false });
				case "POST /api/auth/logout":
					return json(route, 200, { endSessionUrl: null });
				case "POST /api/recordings/init":
					return json(route, 200, {
						sessionId: STUB_IDS.recordingSession,
						fileId: STUB_IDS.recordingFile,
					});
				case "POST /api/recordings/chunk":
					stub.recordingChunks.push({ session, offset, size: bodySize });
					return json(route, 200, { written: offset + bodySize });
				case "POST /api/recordings/finalize":
					return json(route, 200, {
						fileId: "f-e2e-recording",
						name: "Bản ghi e2e",
						durationS: 5,
					});
				case "POST /api/upload/init": {
					const init: unknown = request.postDataJSON();
					uploadSize =
						typeof init === "object" &&
						init !== null &&
						"size" in init &&
						typeof init.size === "number"
							? init.size
							: 0;
					uploaded.set(STUB_IDS.uploadSession, 0);
					return json(route, 200, { sessionId: STUB_IDS.uploadSession });
				}
				case "GET /api/upload/status":
					return json(route, 200, { written: uploaded.get(session) ?? 0 });
				case "POST /api/upload/chunk": {
					// WebKit hides Blob request bodies from Playwright; the app
					// always sends whole chunks, so the size follows from the offset.
					const size =
						bodySize || Math.min(UPLOAD_CHUNK_SIZE, uploadSize - offset);
					uploaded.set(session, offset + size);
					return json(route, 200, { written: offset + size });
				}
				case "POST /api/upload/finalize":
					// Uploads join the history (the record page's "Tải tệp lên").
					historyItems = [
						STUB_UPLOADED,
						...historyItems.filter((item) => item.id !== STUB_UPLOADED.id),
					];
					return json(route, 200, {
						fileId: STUB_UPLOADED.id,
						name: STUB_UPLOADED.name,
						durationS: 754,
					});
				case "POST /api/jobs":
					return json(route, 200, { jobId: STUB_IDS.job });
				case `GET /api/jobs/${STUB_IDS.job}`:
					return json(route, 200, {
						status: "done",
						progress: 100,
						result: { segments: fixtures.splitSegments },
					});
				default:
					if (
						request.method() === "DELETE" &&
						url.pathname.startsWith("/api/files/")
					) {
						stub.deletes.push(url.pathname);
						const id = decodeURIComponent(
							url.pathname.slice("/api/files/".length),
						);
						const known = historyItems.some((item) => item.id === id);
						historyItems = historyItems.filter((item) => item.id !== id);
						return known
							? json(route, 200, { ok: true })
							: json(route, 404, { error: "file không tồn tại" });
					}
					if (
						request.method() === "GET" &&
						url.pathname.startsWith("/api/files/")
					) {
						// Media of a player (`preload="metadata"`): nothing to play.
						return json(route, 404, { error: "Không có tệp (giả lập)" });
					}
					stub.unexpected.push(call);
					return json(route, 404, { error: `Không có stub cho ${call}` });
			}
		},
	);

	return stub;
}
