import { describe, expect, it } from "vitest";
import {
	createSession,
	deleteSession,
	fetchCurrentUser,
} from "../../auth/auth-api";
import { fakeFetch, jsonResponse } from "../../test/fake-fetch";
import {
	deleteFile,
	fetchUsage,
	listManagedFiles,
	listRecordings,
} from "./files-api";

describe("files API", () => {
	it("lists recordings with the retention days", async () => {
		const { fetcher, calls } = fakeFetch({
			"GET /api/files": jsonResponse({
				items: [{ id: "r1", name: "Bản ghi", hasVideo: false, size: 9 }],
				retentionDays: 14,
			}),
		});
		const list = await listRecordings(fetcher);
		expect(list.retentionDays).toBe(14);
		expect(list.items).toHaveLength(1);
		expect(calls[0]).toMatchObject({ method: "GET", credentials: "include" });
	});

	it("defaults to an empty list and unknown retention", async () => {
		const { fetcher } = fakeFetch({ "GET /api/files": jsonResponse({}) });
		expect(await listRecordings(fetcher)).toEqual({
			items: [],
			retentionDays: 0,
		});
	});

	it("lists managed files, reads usage and deletes by id", async () => {
		const { fetcher, calls } = fakeFetch({
			"GET /api/files/manage": jsonResponse({ items: [] }),
			"GET /api/me/usage": jsonResponse({ usageBytes: 1, quotaBytes: 2 }),
			"DELETE /api/files/r1": new Response(null, { status: 204 }),
		});
		expect(await listManagedFiles(fetcher)).toEqual([]);
		expect(await fetchUsage(fetcher)).toEqual({ usageBytes: 1, quotaBytes: 2 });
		await deleteFile("r1", fetcher);
		expect(calls.map((call) => `${call.method} ${call.url}`)).toEqual([
			"GET /api/files/manage",
			"GET /api/me/usage",
			"DELETE /api/files/r1",
		]);
	});
});

describe("auth API", () => {
	it("reads the user, or null without a session", async () => {
		const signedIn = fakeFetch({
			"GET /api/me": jsonResponse({ email: "a@inet.vn" }),
		});
		expect(await fetchCurrentUser(signedIn.fetcher)).toEqual({
			email: "a@inet.vn",
		});
		const anonymous = fakeFetch({ "GET /api/me": jsonResponse({}, 401) });
		expect(await fetchCurrentUser(anonymous.fetcher)).toBeNull();
	});

	it("trades the ID token and surfaces the backend's refusal", async () => {
		const { fetcher, calls } = fakeFetch({
			"POST /api/auth/session": jsonResponse(
				{ error: "Email không được phép" },
				403,
			),
		});
		await expect(createSession("tok", fetcher)).rejects.toMatchObject({
			message: "Email không được phép",
		});
		expect(calls[0]?.body).toEqual({ idToken: "tok" });
	});

	it("logs out even when the backend is unreachable", async () => {
		const { fetcher, calls } = fakeFetch({
			"POST /api/auth/logout": new TypeError("Failed to fetch"),
		});
		await expect(deleteSession(fetcher)).resolves.toBeUndefined();
		expect(calls[0]?.method).toBe("POST");
	});
});
