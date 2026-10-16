import type { Page, Route } from "@playwright/test";
import type { CurrentUser } from "../../src/auth/auth-api";
import type {
	ManagedFile,
	RecordingList,
	Usage,
} from "../../src/features/files/files-api";
import type { TranscriptSummary } from "../../src/features/transcript-docs/transcript-docs-api";
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
	transcripts: Transcript[];
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
		...overrides,
	};
}

export interface ApiStub {
	/** `METHOD /path` of every `/api` request no stub answers. */
	readonly unexpected: string[];
	/** `METHOD /path` of every `/api` request, in order. */
	readonly requests: string[];
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
	const stub: ApiStub = { unexpected: [], requests: [] };

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
				default:
					stub.unexpected.push(call);
					return json(route, 404, { error: `Không có stub cho ${call}` });
			}
		},
	);

	return stub;
}
