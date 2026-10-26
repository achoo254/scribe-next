import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { fakeFetch, jsonResponse } from "../test/fake-fetch";

/** A fresh module per test: the name is read once per page load. */
async function freshModule() {
	vi.resetModules();
	return import("./idp-name");
}

beforeEach(() => {
	vi.unstubAllGlobals();
});

describe("idpNameOf", () => {
	it("keeps a usable displayName only", async () => {
		const { idpNameOf } = await freshModule();
		expect(idpNameOf({ displayName: "  Acme Login " })).toBe("Acme Login");
		expect(idpNameOf({ displayName: "" })).toBeNull();
		expect(idpNameOf({ displayName: 3 })).toBeNull();
		expect(idpNameOf({ displayName: "x".repeat(65) })).toBeNull();
		expect(idpNameOf(null)).toBeNull();
		expect(idpNameOf("Acme")).toBeNull();
	});
});

describe("useIdpName", () => {
	it("starts from the default, then shows the backend's name, read once", async () => {
		const { DEFAULT_IDP_NAME, loadIdpName, useIdpName } = await freshModule();
		const { fetcher, calls } = fakeFetch({
			"GET /api/idp": jsonResponse({ displayName: "Acme" }),
		});
		const { result } = renderHook(() => useIdpName());
		expect(result.current).toBe(DEFAULT_IDP_NAME);
		await act(() => loadIdpName(fetcher));
		expect(result.current).toBe("Acme");
		await loadIdpName(fetcher);
		expect(calls).toHaveLength(1);
		expect(renderHook(() => useIdpName()).result.current).toBe("Acme");
	});

	it("keeps the default when the backend fails or answers nonsense", async () => {
		const failing = await freshModule();
		const down = fakeFetch({ "GET /api/idp": new TypeError("offline") });
		await failing.loadIdpName(down.fetcher);
		expect(renderHook(() => failing.useIdpName()).result.current).toBe(
			"hoandat.com",
		);

		const odd = await freshModule();
		const empty = fakeFetch({
			"GET /api/idp": jsonResponse({ displayName: " " }),
		});
		await odd.loadIdpName(empty.fetcher);
		expect(renderHook(() => odd.useIdpName()).result.current).toBe(
			"hoandat.com",
		);
	});
});
