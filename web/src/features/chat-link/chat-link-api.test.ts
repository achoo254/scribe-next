import { describe, expect, it } from "vitest";
import { fakeFetch, jsonResponse } from "../../test/fake-fetch";
import { fetchChatLink, startChatLink, unlinkChat } from "./chat-link-api";

describe("chat link api", () => {
	it("reads, starts and removes the link", async () => {
		const { fetcher, calls } = fakeFetch({
			"GET /api/chat/link": jsonResponse({
				linked: true,
				linkedAt: "2026-10-16T03:00:00Z",
				enabled: true,
			}),
			"POST /api/chat/link": jsonResponse({
				url: "https://chat.hoandat.com/b/bot_scribe_bot?start=tok",
				expiresAt: "2026-10-16T03:10:00Z",
			}),
			"DELETE /api/chat/link": jsonResponse({ linked: false }),
		});
		expect(await fetchChatLink(fetcher)).toEqual({
			linked: true,
			linkedAt: "2026-10-16T03:00:00Z",
			enabled: true,
		});
		expect((await startChatLink(fetcher)).url).toContain("?start=tok");
		await unlinkChat(fetcher);
		expect(calls.map((call) => call.method)).toEqual(["GET", "POST", "DELETE"]);
		expect(calls.every((call) => call.credentials === "include")).toBe(true);
	});

	it("treats an odd reply as not linked, and an older server as enabled", async () => {
		const { fetcher } = fakeFetch({
			"GET /api/chat/link": jsonResponse({ linked: "yes", linkedAt: 5 }),
		});
		expect(await fetchChatLink(fetcher)).toEqual({
			linked: false,
			linkedAt: null,
			enabled: true,
		});
	});

	it("says when the server takes no links", async () => {
		const { fetcher } = fakeFetch({
			"GET /api/chat/link": jsonResponse({
				linked: false,
				linkedAt: null,
				enabled: false,
			}),
		});
		expect((await fetchChatLink(fetcher)).enabled).toBe(false);
	});

	it("passes on why linking is not available", async () => {
		const { fetcher } = fakeFetch({
			"POST /api/chat/link": jsonResponse(
				{ error: "Liên kết chat chưa được bật trên máy chủ này." },
				503,
			),
		});
		await expect(startChatLink(fetcher)).rejects.toMatchObject({
			status: 503,
			message: "Liên kết chat chưa được bật trên máy chủ này.",
		});
	});
});
