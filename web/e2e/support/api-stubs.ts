import type { Page, Route } from "@playwright/test";
import type { CurrentUser } from "../../src/auth/auth-api";
import type {
	ManagedFile,
	RecordingList,
	Usage,
} from "../../src/features/files/files-api";
import type { JobSegment } from "../../src/features/split/split-job";
import type {
	TranscriptDetail,
	TranscriptSummary,
} from "../../src/features/transcript-docs/transcript-docs-api";
import type { Transcript } from "../../src/features/transcript/transcript-api";
import { E2E_ORIGIN } from "./e2e-origin";
import { E2E_STUB_MARKER } from "./stub-markers";

/**
 * What the stubbed backend answers. Shapes come from the app's own types,
 * so a change of the API contract fails `npm run typecheck` here too.
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
}

/** Every date in the fixtures is this day (Asia/Saigon); specs freeze the clock to it. */
export const FIXED_NOW = new Date("2026-10-16T10:00:00+07:00");

export const SIGNED_IN_USER: CurrentUser = {
	email: "minh.anh@example.com",
	canTranscribe: true,
	transcription: {
		plan: "free",
		planUntil: null,
		usedMinutes: 42,
		limitMinutes: 300,
		resetsAt: "2026-11-01T00:00:00+07:00",
		capReached: false,
		paused: false,
	},
};

const IPHONE_SESSION = "a1b2c3d4-e5f6";

export function apiFixtures(overrides: Partial<ApiFixtures> = {}): ApiFixtures {
	return {
		user: SIGNED_IN_USER,
		usage: { usageBytes: 1_288_490_189, quotaBytes: 10_737_418_240 },
		recordings: {
			retentionDays: 30,
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
}

/** Number of `METHOD /path` calls the stub answered. */
export function countOf(stub: ApiStub, call: string): number {
	return stub.requests.filter((request) => request === call).length;
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
	const stub: ApiStub = { unexpected: [], requests: [], recordingChunks: [] };
	// Bytes the stubbed backend holds per upload session ("Cắt file"), and
	// the size announced at init.
	const uploaded = new Map<string, number>();
	let uploadSize = 0;

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
				case "GET /api/files":
					return json(route, 200, fixtures.recordings);
				case "GET /api/files/manage":
					return json(route, 200, { items: fixtures.managedFiles });
				case "GET /api/transcript-docs":
					return json(route, 200, { items: fixtures.transcriptDocs });
				case "GET /api/transcripts": {
					const ids = new Set(
						(url.searchParams.get("ids") ?? "").split(",").filter(Boolean),
					);
					return json(route, 200, {
						items: fixtures.transcripts.filter((t) => ids.has(t.fileId)),
					});
				}
				case "GET /api/auth/login":
					// Stands in for the redirect to hoandat-auth.
					return route.fulfill({
						status: 200,
						contentType: "text/html; charset=utf-8",
						body: `<!doctype html><html lang="vi"><title>${E2E_STUB_MARKER}</title><p>Trang đăng nhập hoandat (giả lập)</p></html>`,
					});
				case "POST /api/auth/logout":
					return json(route, 200, { endSessionUrl: null });
				case "POST /api/recordings/init":
					return json(route, 200, { sessionId: STUB_IDS.recordingSession });
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
					return json(route, 200, {
						fileId: "f-e2e-upload",
						name: "phong-van.m4a",
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
