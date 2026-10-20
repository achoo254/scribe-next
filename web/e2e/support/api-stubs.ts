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
	TranscriptTranslation,
	TranslationProgress,
} from "../../src/features/transcript-docs/transcript-docs-api";
import type { Transcript } from "../../src/features/transcript/transcript-api";
import {
	type KeyProvider,
	type KeyUse,
	type StoredKey,
	USE_PROVIDERS,
} from "../../src/features/user-keys/key-uses";
import type {
	KeyChoice,
	UserKeys,
} from "../../src/features/user-keys/user-keys-api";
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
	/**
	 * What `GET /api/transcript-docs/:key/translation` answers once a run
	 * started by `POST …/translation` has ended (its `lang` is the one asked).
	 */
	translation: TranscriptTranslation | null;
	/** "Key riêng" as `GET /api/me/keys` answers before the spec changes it. */
	userKeys: UserKeys;
	/** Replies that win over the stub's own, by `METHOD /path` (e.g. an error). */
	replies: Record<string, StubReply>;
}

const NO_CHOICE: KeyChoice = {
	transcribe: null,
	live: null,
	text: null,
	textModel: null,
};

/** The text models `GET /api/me/keys` lists (the backend's short list). */
export const STUB_TEXT_MODELS: UserKeys["models"] = {
	openai: [
		{ id: "gpt-4.1-mini", label: "GPT-4.1 mini" },
		{ id: "gpt-4.1-nano", label: "GPT-4.1 nano (rẻ hơn)" },
	],
	gemini: [
		{ id: "gemini-3.8-flash", label: "Gemini 3.8 Flash" },
		{ id: "gemini-3.5-flash-lite", label: "Gemini 3.5 Flash-Lite (rẻ hơn)" },
	],
	anthropic: [
		{ id: "claude-haiku-5-5", label: "Claude Haiku 5.5" },
		{ id: "claude-sonnet-5-5", label: "Claude Sonnet 5.5 (tốt hơn, đắt hơn)" },
	],
};

/** A key the stubbed provider refuses on save. */
export const REJECTED_KEY = "sk-test-rejected-000000";

/** What runs on which key: the user's pick when valid, else the first valid one (as the backend). */
function activeChoice(keys: StoredKey[], prefs: KeyChoice): KeyChoice {
	const valid = new Set(
		keys.filter((k) => k.status === "valid").map((k) => k.provider),
	);
	const pick = (use: KeyUse): KeyProvider | null => {
		const chosen = prefs[use];
		if (chosen && valid.has(chosen)) return chosen;
		return USE_PROVIDERS[use].find((p) => valid.has(p)) ?? null;
	};
	const text = pick("text");
	const models = text ? (STUB_TEXT_MODELS[text] ?? []) : [];
	return {
		transcribe: pick("transcribe"),
		live: pick("live"),
		text,
		textModel: text
			? (models.find((m) => m.id === prefs.textModel)?.id ??
				models[0]?.id ??
				null)
			: null,
	};
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
		translation: null,
		userKeys: {
			available: true,
			keys: [],
			prefs: NO_CHOICE,
			active: NO_CHOICE,
			models: STUB_TEXT_MODELS,
		},
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
	/** `lang` of every `POST /api/transcript-docs/:key/translation`, in order. */
	readonly translations: string[];
	/** Query of every `GET /api/transcripts/export`, in order. */
	readonly exports: URLSearchParams[];
	/** `names` of every `PATCH /api/transcript-docs/:key/speakers`, in order. */
	readonly speakerRenames: Array<Record<string, string | null>>;
	/** Provider of every `PUT /api/me/keys/:provider`, in order (the key itself is not kept). */
	readonly keySaves: string[];
	/** `confirm` of every `POST /api/me/delete` (account deletion), in order. */
	readonly accountDeletes: string[];
	/** Stands in for the user sending `/start` to the bot on chat.hoandat.com. */
	linkChat: () => void;
}

/** Number of `METHOD /path` calls the stub answered. */
export function countOf(stub: ApiStub, call: string): number {
	return stub.requests.filter((request) => request === call).length;
}

/** `/api/transcript-docs/<key>/<summary|share|bookmarks|translation|speakers>[/<id>]`. */
const DOC_ACTION =
	/^\/api\/transcript-docs\/([^/]+)\/(summary|share|bookmarks|translation|speakers)(?:\/([^/]+))?$/;

/** `names` of a speaker rename body: id → name, or null (back to "Người N"). */
function speakerNames(body: unknown): Record<string, string | null> {
	const names: unknown =
		typeof body === "object" && body !== null
			? Reflect.get(body, "names")
			: null;
	const out: Record<string, string | null> = {};
	if (typeof names !== "object" || names === null) return out;
	for (const [id, name] of Object.entries(names)) {
		out[id] = typeof name === "string" ? name : null;
	}
	return out;
}

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
	// `POST /api/me/delete` ends the session: `/api/me` answers 401 from then on.
	let user = fixtures.user;
	const stub: ApiStub = {
		unexpected: [],
		requests: [],
		recordingChunks: [],
		bookmarks: [],
		searches: [],
		questions: [],
		transcribed: [],
		deletes: [],
		translations: [],
		exports: [],
		speakerRenames: [],
		keySaves: [],
		accountDeletes: [],
		linkChat: () => {
			chatLinked = true;
		},
	};
	// Bytes the stubbed backend holds per upload session ("Cắt file"), and
	// the size announced at init.
	const uploaded = new Map<string, number>();
	let uploadSize = 0;
	// Only the record page's "Tải tệp lên" asks for a history file.
	let uploadForHistory = false;
	// "Lịch sử ghi" and transcripts change as the spec uploads, transcribes
	// and deletes.
	let historyItems: RecordingItem[] = [...fixtures.recordings.items];
	// "Key riêng" changes as the spec saves, deletes and picks.
	let userKeys: UserKeys = {
		...fixtures.userKeys,
		active: activeChoice(fixtures.userKeys.keys, fixtures.userKeys.prefs),
	};
	const setKeys = (keys: StoredKey[], prefs: KeyChoice = userKeys.prefs) => {
		userKeys = { ...userKeys, keys, prefs, active: activeChoice(keys, prefs) };
	};
	const transcripts = [...fixtures.transcripts];
	// Transcripts whose speakers the spec renames.
	const details: Record<string, TranscriptDetail> = {
		...fixtures.transcriptDetails,
	};
	// A translation run: started by the POST, reported running by the first
	// status poll, done from the second on.
	let translationRun: { lang: "en" | "vi"; polls: number } | null = null;
	const translationProgress = (): TranslationProgress | null => {
		if (!translationRun || !fixtures.translation) return null;
		const total = fixtures.translation.total;
		return translationRun.polls < 2
			? {
					lang: translationRun.lang,
					status: "running",
					done: translationRun.polls,
					total,
					error: null,
				}
			: {
					lang: translationRun.lang,
					status: "done",
					done: total,
					total,
					error: null,
				};
	};
	/** `POST …/translation`, `GET …/translation[/status]?lang=`. */
	const translationReply = (
		route: Route,
		method: string,
		url: URL,
		sub: string | undefined,
	): Promise<void> => {
		const request = route.request();
		if (method === "POST") {
			const asked = stringField(request.postDataJSON(), "lang");
			stub.translations.push(asked ?? "");
			translationRun = { lang: asked === "vi" ? "vi" : "en", polls: 0 };
			return json(route, 202, translationProgress());
		}
		const lang = url.searchParams.get("lang") === "vi" ? "vi" : "en";
		const progress =
			translationRun?.lang === lang ? translationProgress() : null;
		if (sub === "status") {
			if (translationRun) translationRun.polls += 1;
			const none: TranslationProgress = {
				lang,
				status: "none",
				done: 0,
				total: 0,
				error: null,
			};
			return json(route, 200, progress ?? none);
		}
		if (progress && fixtures.translation) {
			const body: TranscriptTranslation =
				progress.status === "done"
					? { ...fixtures.translation, lang }
					: { ...fixtures.translation, ...progress, segments: [] };
			return json(route, 200, body);
		}
		const none: TranscriptTranslation = {
			lang,
			status: "none",
			done: 0,
			total: 0,
			error: null,
			model: null,
			updatedAt: null,
			outdated: 0,
			missing: 0,
			segments: [],
		};
		return json(route, 200, none);
	};

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
			const keyPath = /^\/api\/me\/keys\/([a-z]+)$/.exec(url.pathname);
			if (keyPath) {
				const provider = keyPath[1] as KeyProvider;
				const others = userKeys.keys.filter((k) => k.provider !== provider);
				if (request.method() === "DELETE") {
					setKeys(others);
					return json(route, 200, { deleted: true });
				}
				stub.keySaves.push(provider);
				const key = stringField(request.postDataJSON(), "key") ?? "";
				if (key === REJECTED_KEY) {
					return json(route, 400, {
						error:
							"OpenAI từ chối key này: key sai, đã bị thu hồi hoặc thiếu quyền.",
						code: "KEY_REJECTED",
					});
				}
				const stored: StoredKey = {
					provider,
					last4: key.slice(-4),
					status: "valid",
					updatedAt: FIXED_NOW.toISOString(),
					lastCheckedAt: FIXED_NOW.toISOString(),
				};
				setKeys([...others, stored]);
				return json(route, 200, stored);
			}
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
				if (action === "translation") {
					return translationReply(route, request.method(), url, id);
				}
				const doc = details[key];
				if (request.method() === "PATCH" && action === "speakers" && doc) {
					const renames = speakerNames(request.postDataJSON());
					stub.speakerRenames.push(renames);
					const names = { ...(doc.speakers?.names ?? {}) };
					for (const [speaker, name] of Object.entries(renames)) {
						names[speaker] = name ?? `Người ${Number(speaker.slice(1)) + 1}`;
					}
					details[key] = { ...doc, speakers: { names } };
					return json(route, 200, { names });
				}
			}
			if (
				request.method() === "GET" &&
				url.pathname.startsWith("/api/transcript-docs/")
			) {
				const key = decodeURIComponent(
					url.pathname.slice("/api/transcript-docs/".length),
				);
				const detail = details[key];
				const progress = translationProgress();
				if (detail && progress) {
					return json(route, 200, {
						...detail,
						translations: [{ lang: progress.lang, status: progress.status }],
					});
				}
				if (detail) return json(route, 200, detail);
			}
			switch (call) {
				case "POST /api/me/delete": {
					stub.accountDeletes.push(
						stringField(request.postDataJSON(), "confirm") ?? "",
					);
					user = null;
					return json(route, 200, {
						deleted: true,
						files: historyItems.length,
						bytes: 0,
						complete: true,
					});
				}
				// The IdP's default tenant name, as the app shows before the reply,
				// so screenshots do not depend on when it lands.
				case "GET /api/idp":
					return json(route, 200, { displayName: "Auth Next" });
				case "GET /api/me":
					return user
						? json(route, 200, {
								...user,
								ownKeys: {
									available: userKeys.available,
									...userKeys.active,
									minutes: 0,
								},
							})
						: json(route, 401, { error: "Chưa đăng nhập" });
				case "GET /api/me/keys":
					return json(route, 200, userKeys);
				case "PUT /api/me/key-prefs": {
					const body: unknown = request.postDataJSON();
					const provider = (name: KeyUse): KeyProvider | null =>
						(stringField(body, name) as KeyProvider | null) ?? null;
					setKeys(userKeys.keys, {
						transcribe: provider("transcribe"),
						live: provider("live"),
						text: provider("text"),
						textModel: stringField(body, "textModel"),
					});
					return json(route, 200, userKeys);
				}
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
				case "GET /api/transcripts/export":
					stub.exports.push(url.searchParams);
					// An attachment: the browser saves it and stays on the page.
					return route.fulfill({
						status: 200,
						contentType: "text/markdown; charset=utf-8",
						headers: {
							"Content-Disposition": 'attachment; filename="ban-dich.md"',
						},
						body: "# Bản dịch (giả lập)\n",
					});
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
					uploadForHistory = stringField(init, "purpose") === "transcribe";
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
					// Like the backend: the record page's "Tải tệp lên" joins the
					// history, a "Cắt file" upload does not.
					if (uploadForHistory) {
						historyItems = [
							STUB_UPLOADED,
							...historyItems.filter((item) => item.id !== STUB_UPLOADED.id),
						];
					}
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
