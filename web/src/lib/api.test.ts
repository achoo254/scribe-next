import { afterEach, describe, expect, it } from "vitest";
import { fakeFetch, jsonResponse } from "../test/fake-fetch";
import {
	ApiError,
	NATIVE_API_ORIGIN,
	apiBase,
	apiFetch,
	apiJson,
	apiUrl,
	isNativeShell,
	nativeSafeBody,
} from "./api";

afterEach(() => {
	Reflect.deleteProperty(globalThis, "Capacitor");
});

describe("API base", () => {
	it("keeps /api paths relative on the web", () => {
		expect(isNativeShell()).toBe(false);
		expect(apiBase()).toBe("");
		expect(apiUrl("/api/me")).toBe("/api/me");
	});

	it("prefixes the staging origin in a native shell", () => {
		Reflect.set(globalThis, "Capacitor", { isNativePlatform: () => true });
		expect(isNativeShell()).toBe(true);
		expect(apiUrl("/api/files/a1")).toBe(`${NATIVE_API_ORIGIN}/api/files/a1`);
	});

	it("treats Capacitor's web build as the web", () => {
		Reflect.set(globalThis, "Capacitor", { isNativePlatform: () => false });
		expect(apiBase()).toBe("");
	});

	it("rejects paths outside /api/", () => {
		expect(() => apiUrl("/login")).toThrow();
	});
});

describe("apiFetch", () => {
	it("sends the cookie and a JSON body", async () => {
		const { fetcher, calls } = fakeFetch({
			"POST /api/auth/session": jsonResponse({ email: "a@example.com" }),
		});
		const user = await apiJson(
			"/api/auth/session",
			{ method: "POST", json: { idToken: "t" } },
			fetcher,
		);
		expect(user).toEqual({ email: "a@example.com" });
		expect(calls[0]).toMatchObject({
			method: "POST",
			url: "/api/auth/session",
			body: { idToken: "t" },
			credentials: "include",
			contentType: "application/json",
		});
	});

	it("throws the backend's { error } message with the status", async () => {
		const { fetcher } = fakeFetch({
			"POST /api/upload/init": jsonResponse({ error: "Vượt hạn mức" }, 413),
		});
		const failure = apiFetch("/api/upload/init", { method: "POST" }, fetcher);
		await expect(failure).rejects.toEqual(new ApiError(413, "Vượt hạn mức"));
	});

	it("keeps the machine code and the quota numbers of a refusal", async () => {
		const { fetcher } = fakeFetch({
			"POST /api/x": jsonResponse(
				{
					error: "Hết lượt",
					code: "SUMMARY_DAILY_LIMIT",
					usedMinutes: 10,
					limitMinutes: 10,
					retryAt: "2026-10-16T17:00:00+00:00",
				},
				403,
			),
		});
		const failure = apiFetch("/api/x", { method: "POST" }, fetcher);
		await expect(failure).rejects.toMatchObject({
			status: 403,
			message: "Hết lượt",
			code: "SUMMARY_DAILY_LIMIT",
			quota: { used: 10, limit: 10, retryAt: "2026-10-16T17:00:00+00:00" },
		});
	});

	it("has no code or quota when the reply carries none", async () => {
		const { fetcher } = fakeFetch({
			"POST /api/x": jsonResponse({ error: "Sai", retryAt: null }, 400),
		});
		await expect(
			apiFetch("/api/x", { method: "POST" }, fetcher),
		).rejects.toMatchObject({ code: null, quota: null });
	});

	it("falls back to the given message when the body has none", async () => {
		const { fetcher } = fakeFetch({
			"GET /api/files": new Response("oops", { status: 502 }),
		});
		await expect(
			apiFetch("/api/files", { fallbackError: "Không tải được" }, fetcher),
		).rejects.toMatchObject({ status: 502, message: "Không tải được" });
	});

	it("turns a network failure into status 0 with a Vietnamese message", async () => {
		const { fetcher } = fakeFetch({
			"GET /api/me": new TypeError("Failed to fetch"),
		});
		await expect(apiFetch("/api/me", {}, fetcher)).rejects.toMatchObject({
			status: 0,
			message: expect.stringContaining("Không kết nối được máy chủ"),
		});
	});
});

describe("nativeSafeBody", () => {
	const chunk = new Blob([new Uint8Array([0, 1, 2, 255])]).slice(1, 4);

	it("keeps a Blob as it is on the web", () => {
		expect(nativeSafeBody(chunk, null, false)).toBe(chunk);
	});

	it("sends a Blob as a File with the same bytes in the native shell", async () => {
		const body = nativeSafeBody(chunk, "application/octet-stream", true);
		expect(body).toBeInstanceOf(File);
		expect((body as File).type).toBe("application/octet-stream");
		// jsdom's File has no arrayBuffer(); FileReader reads it.
		const buffer = await new Promise<ArrayBuffer>((resolve, reject) => {
			const reader = new FileReader();
			reader.onload = () => resolve(reader.result as ArrayBuffer);
			reader.onerror = () => reject(reader.error);
			reader.readAsArrayBuffer(body as File);
		});
		const bytes = new Uint8Array(buffer);
		expect([...bytes]).toEqual([1, 2, 255]);
	});

	it("leaves Files, strings and empty bodies alone", () => {
		const file = new File(["a"], "a.m4a");
		expect(nativeSafeBody(file, null, true)).toBe(file);
		expect(nativeSafeBody("{}", null, true)).toBe("{}");
		expect(nativeSafeBody(undefined, null, true)).toBeUndefined();
	});

	it("wraps the chunk apiFetch sends from the native shell", async () => {
		Reflect.set(globalThis, "Capacitor", { isNativePlatform: () => true });
		const sent: unknown[] = [];
		const fetcher: typeof fetch = async (_url, init) => {
			sent.push(init?.body);
			return new Response("{}", { status: 200 });
		};
		await apiFetch(
			"/api/upload/chunk",
			{ method: "POST", body: chunk },
			fetcher,
		);
		expect(sent[0]).toBeInstanceOf(File);
	});
});
