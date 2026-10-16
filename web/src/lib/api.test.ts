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
