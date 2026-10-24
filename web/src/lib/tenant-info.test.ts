import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { fakeFetch, jsonResponse } from "../test/fake-fetch";

/** A fresh store per test: the tenant is read once per page load. */
async function fresh() {
	vi.resetModules();
	const info = await import("./tenant-info");
	const api = await import("./api");
	const idp = await import("./idp");
	return { ...info, api, idp };
}

const ACME = {
	displayName: "Acme",
	domain: "acme.vn",
	issuer: "https://auth.acme.vn",
	primary: false,
	scribe: "enabled",
};

beforeEach(() => {
	vi.unstubAllGlobals();
});

describe("tenantInfoOf", () => {
	it("reads another tenant and falls back to the primary one on anything missing", async () => {
		const { tenantInfoOf, DEFAULT_TENANT_INFO } = await fresh();
		expect(tenantInfoOf(ACME)).toEqual(ACME);
		expect(tenantInfoOf({ ...ACME, scribe: "disabled" }).scribe).toBe(
			"disabled",
		);
		// An unknown state is "not enabled": never show sign-in by mistake.
		expect(tenantInfoOf({ ...ACME, scribe: "maybe" }).scribe).toBe("disabled");
		// An older backend: the name only.
		expect(tenantInfoOf({ displayName: "Auth Next" })).toEqual(
			DEFAULT_TENANT_INFO,
		);
		expect(tenantInfoOf({ ...ACME, primary: true })).toEqual({
			...DEFAULT_TENANT_INFO,
			displayName: "Acme",
		});
		for (const broken of [
			{ ...ACME, domain: 3 },
			{ ...ACME, domain: "not a domain" },
			{ ...ACME, issuer: "javascript:alert(1)" },
			{ ...ACME, primary: "no" },
		]) {
			expect(tenantInfoOf(broken).primary).toBe(true);
		}
		expect(tenantInfoOf(null)).toEqual(DEFAULT_TENANT_INFO);
		expect(tenantInfoOf("Acme")).toEqual(DEFAULT_TENANT_INFO);
		expect(DEFAULT_TENANT_INFO).toMatchObject({
			domain: "hoandat.com",
			primary: true,
			scribe: "enabled",
		});
	});

	it("names the domain of an unknown host", async () => {
		const { unknownTenantInfo } = await fresh();
		expect(unknownTenantInfo("Scribe.Acme.vn")).toEqual({
			displayName: "Auth Next",
			domain: "acme.vn",
			issuer: "https://auth.acme.vn",
			primary: false,
			scribe: "disabled",
		});
	});
});

describe("IdP links", () => {
	it("use the build origin on the primary tenant and the issuer elsewhere", async () => {
		const { idp, DEFAULT_TENANT_INFO } = await fresh();
		expect(idp.idpAccountUrlOf(DEFAULT_TENANT_INFO)).toBe(idp.IDP_ACCOUNT_URL);
		expect(idp.idpAppsUrlOf(DEFAULT_TENANT_INFO)).toBe(idp.IDP_APPS_URL);
		const acme = { ...ACME, scribe: "enabled" as const };
		expect(idp.idpAccountUrlOf(acme)).toBe("https://auth.acme.vn/account");
		expect(idp.idpAppsUrlOf(acme)).toBe(
			"https://auth.acme.vn/account#account-apps",
		);
		expect(idp.idpAdminUrlOf(acme)).toBe("https://auth.acme.vn/admin");
	});
});

describe("loadTenantInfo", () => {
	it("reads /api/idp once and settles", async () => {
		const { loadTenantInfo, useTenantInfo, useTenantSettled } = await fresh();
		const { fetcher, calls } = fakeFetch({
			"GET /api/idp": jsonResponse(ACME),
		});
		const settled = renderHook(() => useTenantSettled());
		const info = renderHook(() => useTenantInfo());
		expect(settled.result.current).toBe(false);
		expect(info.result.current.primary).toBe(true);
		await act(() => loadTenantInfo(fetcher));
		await loadTenantInfo(fetcher);
		expect(calls).toHaveLength(1);
		expect(settled.result.current).toBe(true);
		expect(info.result.current).toEqual(ACME);
	});

	it("shows an unknown host as not enabled and keeps the defaults when the backend fails", async () => {
		const unknown = await fresh();
		const notServed = fakeFetch({
			"GET /api/idp": jsonResponse(
				{ error: "Scribe không phục vụ domain này", code: "UNKNOWN_TENANT" },
				404,
			),
		});
		await unknown.loadTenantInfo(notServed.fetcher);
		const shown = renderHook(() => unknown.useTenantInfo()).result.current;
		expect({ primary: shown.primary, scribe: shown.scribe }).toEqual({
			primary: false,
			scribe: "disabled",
		});

		const down = await fresh();
		const offline = fakeFetch({ "GET /api/idp": new TypeError("offline") });
		await down.loadTenantInfo(offline.fetcher);
		expect(renderHook(() => down.useTenantSettled()).result.current).toBe(true);
		expect(renderHook(() => down.useTenantInfo()).result.current).toEqual(
			down.DEFAULT_TENANT_INFO,
		);
	});

	it("switches a tenant to not enabled when a request says so, never the primary tenant", async () => {
		const tenant = await fresh();
		await tenant.loadTenantInfo(
			fakeFetch({ "GET /api/idp": jsonResponse(ACME) }).fetcher,
		);
		const disabled = jsonResponse(
			{ error: "Scribe chưa bật trên domain này", code: "TENANT_DISABLED" },
			403,
		);
		const me = fakeFetch({ "GET /api/me": disabled });
		const view = renderHook(() => tenant.useTenantInfo());
		await act(async () => {
			await expect(
				tenant.api.apiFetch("/api/me", {}, me.fetcher),
			).rejects.toThrow();
		});
		expect(view.result.current.scribe).toBe("disabled");

		const primary = await fresh();
		await primary.loadTenantInfo(
			fakeFetch({ "GET /api/idp": jsonResponse({ displayName: "Auth Next" }) })
				.fetcher,
		);
		const again = fakeFetch({ "GET /api/me": disabled });
		await expect(
			primary.api.apiFetch("/api/me", {}, again.fetcher),
		).rejects.toThrow();
		expect(
			renderHook(() => primary.useTenantInfo()).result.current.scribe,
		).toBe("enabled");
	});
});

describe("a tenant's host whose /api/idp fails", () => {
	it("is unavailable, never the primary tenant", async () => {
		const { isTenantHost, loadTenantInfo, useTenantInfo } = await fresh();
		expect(isTenantHost("scribe.acme.vn")).toBe(true);
		expect(isTenantHost("Scribe.Hoandat.com")).toBe(false);
		expect(isTenantHost("localhost")).toBe(false);
		vi.stubGlobal("location", { hostname: "scribe.acme.vn" });
		const down = fakeFetch({
			"GET /api/idp": jsonResponse({ error: "bận" }, 503),
		});
		await loadTenantInfo(down.fetcher);
		expect(renderHook(() => useTenantInfo()).result.current).toMatchObject({
			domain: "acme.vn",
			primary: false,
			scribe: "unavailable",
		});
	});
});
