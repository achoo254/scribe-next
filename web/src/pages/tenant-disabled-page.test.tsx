import { act, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { fakeFetch, jsonResponse } from "../test/fake-fetch";

/** A fresh tenant store and the modules reading it. */
async function fresh() {
	vi.resetModules();
	const info = await import("../lib/tenant-info");
	const app = await import("../app");
	const page = await import("./tenant-disabled-page");
	// Providers from the same fresh module graph as the components.
	const render = await import("../test/render-with-providers");
	return { ...info, ...app, ...page, ...render };
}

const ACME = {
	displayName: "Acme",
	domain: "acme.vn",
	issuer: "https://auth.acme.vn",
	primary: false,
};

describe("TenantDisabledPage", () => {
	it("names the domain and sends its owner to the IdP's admin page", async () => {
		const { TenantDisabledPage, renderWithProviders } = await fresh();
		renderWithProviders(
			<TenantDisabledPage tenant={{ ...ACME, scribe: "disabled" }} />,
		);
		expect(
			screen.getByRole("heading", { name: "Scribe chưa bật trên domain này" }),
		).toBeVisible();
		expect(screen.getByText("Domain: acme.vn")).toBeVisible();
		expect(
			screen.getByText(/Chủ domain acme.vn chưa bật Scribe/),
		).toBeVisible();
		const admin = screen.getByRole("link", { name: /trang quản trị acme.vn/ });
		expect(admin).toHaveAttribute("href", "https://auth.acme.vn/admin");
		expect(admin).toHaveAttribute("target", "_blank");
	});
});

describe("TenantGate", () => {
	it("waits for the tenant, then shows the app or the not enabled page", async () => {
		const { TenantGate, loadTenantInfo, renderWithProviders } = await fresh();
		renderWithProviders(
			<TenantGate>
				<p>ứng dụng</p>
			</TenantGate>,
		);
		// While /api/idp has not answered: the session check, not the app.
		expect(screen.getByText("Đang kiểm tra phiên đăng nhập…")).toBeVisible();
		expect(screen.queryByText("ứng dụng")).toBeNull();
		const { fetcher } = fakeFetch({
			"GET /api/idp": jsonResponse({ ...ACME, scribe: "disabled" }),
		});
		await act(() => loadTenantInfo(fetcher));
		expect(
			screen.getByRole("heading", { name: "Scribe chưa bật trên domain này" }),
		).toBeVisible();
		expect(screen.queryByText("ứng dụng")).toBeNull();
	});

	it("shows the app on an enabled tenant and on the primary one", async () => {
		for (const body of [
			{ ...ACME, scribe: "enabled" },
			{ displayName: "Auth Next" },
		]) {
			const { TenantGate, loadTenantInfo, renderWithProviders } = await fresh();
			const { fetcher } = fakeFetch({ "GET /api/idp": jsonResponse(body) });
			await loadTenantInfo(fetcher);
			const view = renderWithProviders(
				<TenantGate>
					<p>ứng dụng</p>
				</TenantGate>,
			);
			expect(screen.getByText("ứng dụng")).toBeVisible();
			view.unmount();
		}
	});
});
