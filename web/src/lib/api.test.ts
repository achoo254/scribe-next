import { afterEach, describe, expect, it } from "vitest";
import { setActiveTranslator } from "../i18n/active-translator";
import { VIETNAMESE } from "../i18n/i18n-context";
import { en } from "../i18n/messages/en/dictionary";
import { zhCN } from "../i18n/messages/zh-CN/dictionary";
import { createTranslator } from "../i18n/translate";
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
	setActiveTranslator(VIETNAMESE);
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

describe("apiFetch on an English page", () => {
	const english = (): void => setActiveTranslator(createTranslator("en", en));

	it("words a coded refusal in English instead of the backend's Vietnamese", async () => {
		english();
		const { fetcher } = fakeFetch({
			"POST /api/transcripts": jsonResponse(
				{
					error: "Không đủ phút chép lời: tháng này đã dùng 120/120 phút.",
					code: "QUOTA_EXCEEDED",
					usedMinutes: 120,
					limitMinutes: 120,
					retryAt: "2026-11-01T00:00:00+07:00",
				},
				403,
			),
		});
		await expect(
			apiFetch("/api/transcripts", { method: "POST" }, fetcher),
		).rejects.toMatchObject({
			status: 403,
			code: "QUOTA_EXCEEDED",
			message:
				"Not enough transcription minutes: you've used 120/120 minutes this month.",
		});
	});

	it("says when a limit opens again, in English", async () => {
		english();
		const { fetcher } = fakeFetch({
			"POST /api/x": jsonResponse(
				{
					error:
						"Tài khoản mới dùng được chép lời miễn phí từ 09:00 ngày 29/10.",
					code: "ACCOUNT_TOO_NEW",
					usedMinutes: 0,
					limitMinutes: 0,
					retryAt: "2026-10-29T02:00:00Z",
				},
				403,
			),
		});
		await expect(
			apiFetch("/api/x", { method: "POST" }, fetcher),
		).rejects.toMatchObject({
			message: "New accounts can use free transcription from 09:00, Oct 29.",
		});
	});

	it("words a full storage (413) without a code", async () => {
		english();
		const { fetcher } = fakeFetch({
			"POST /api/upload/init": jsonResponse(
				{ error: "Đã hết dung lượng" },
				413,
			),
		});
		await expect(
			apiFetch("/api/upload/init", { method: "POST" }, fetcher),
		).rejects.toMatchObject({
			status: 413,
			message: expect.stringContaining("your storage is full"),
		});
	});

	it("shows the request's English fallback for any other refusal", async () => {
		english();
		const { fetcher } = fakeFetch({
			"PATCH /api/x": jsonResponse({ error: "tên quá dài" }, 400),
		});
		await expect(
			apiFetch(
				"/api/x",
				{ method: "PATCH", fallbackError: "Couldn't rename" },
				fetcher,
			),
		).rejects.toMatchObject({ status: 400, message: "Couldn't rename" });
	});

	it("keeps the backend's text on a Vietnamese page", async () => {
		const { fetcher } = fakeFetch({
			"PATCH /api/x": jsonResponse({ error: "tên quá dài" }, 400),
		});
		await expect(
			apiFetch("/api/x", { method: "PATCH" }, fetcher),
		).rejects.toMatchObject({ message: "tên quá dài" });
	});
});

describe("apiFetch on a Chinese page", () => {
	it("words a coded refusal in Chinese instead of the backend's Vietnamese", async () => {
		setActiveTranslator(createTranslator("zh-CN", zhCN));
		const { fetcher } = fakeFetch({
			"POST /api/x": jsonResponse(
				{
					error:
						"Tài khoản mới dùng được chép lời miễn phí từ 09:00 ngày 29/10.",
					code: "ACCOUNT_TOO_NEW",
					usedMinutes: 0,
					limitMinutes: 0,
					retryAt: "2026-10-29T02:00:00Z",
				},
				403,
			),
		});
		await expect(
			apiFetch("/api/x", { method: "POST" }, fetcher),
		).rejects.toMatchObject({
			message: "新账号可从 10月29日 09:00 起使用免费转写。",
		});
	});
});
