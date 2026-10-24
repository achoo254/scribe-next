import { screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { CurrentUser } from "../auth/auth-api";
import { fakeFetch, jsonResponse } from "../test/fake-fetch";
import { renderWithProviders } from "../test/render-with-providers";
import { UsageCard, tenantKeyRows, usageTone } from "./usage-card";

const auth = vi.hoisted(() => ({ user: null as CurrentUser | null }));
vi.mock("../auth/auth-context", () => ({
	useAuth: () => ({ user: auth.user }),
}));
// The tenant of the host: the primary one unless a test says otherwise.
const tenant = vi.hoisted(() => ({ domain: "hoandat.com", primary: true }));
vi.mock("../lib/tenant-info", () => ({
	useTenantInfo: () => ({
		displayName: "Auth Next",
		issuer: "https://auth.hoandat.com",
		scribe: "enabled",
		...tenant,
	}),
}));

const MB = 1024 * 1024;

const FREE: CurrentUser = {
	email: "a@b.c",
	canTranscribe: true,
	plan: {
		name: "free",
		until: null,
		quotaBytes: 200 * MB,
		maxFileBytes: 200 * MB,
		retentionDays: 7,
	},
	transcription: {
		plan: "free",
		usedMinutes: 5,
		limitMinutes: 120,
		resetsAt: "2026-10-31T17:00:00Z",
		capReached: false,
		daily: { usedMinutes: 28, limitMinutes: 30 },
	},
};

function renderCard(user: CurrentUser, usageBytes: number, quotaBytes: number) {
	auth.user = user;
	const fake = fakeFetch({
		"GET /api/me/usage": jsonResponse({ usageBytes, quotaBytes }),
	});
	vi.stubGlobal("fetch", fake.fetcher);
	renderWithProviders(<UsageCard />);
}

afterEach(() => {
	vi.unstubAllGlobals();
});

describe("UsageCard", () => {
	it("shows the free plan, storage, this month's and today's minutes on one card", async () => {
		renderCard(FREE, 50 * MB, 200 * MB);
		const card = await screen.findByRole("region", { name: "Gói và mức dùng" });
		expect(within(card).getByText("Gói Free")).toBeVisible();
		expect(card).toHaveTextContent("Tệp tự xoá sau 7 ngày");
		expect(card).toHaveTextContent("phút đặt lại ngày 1/11");
		expect(card).toHaveTextContent("Lưu trữ50 MB / 200 MB");
		expect(card).toHaveTextContent("Chép lời tháng này5/120 phút");
		expect(card).toHaveTextContent("Hôm nay28/30 phút");
		const today = within(card).getByRole("meter", {
			name: "Phút chép lời đã dùng hôm nay",
		});
		expect(today).toHaveAttribute("data-tone", "error");
		expect(screen.queryByText(/Đã đầy dung lượng/)).toBeNull();
	});

	it("says storage is full when the plan's limit is reached", async () => {
		renderCard({ ...FREE, transcription: null }, 230 * MB, 200 * MB);
		expect(
			await screen.findByText(
				"Đã đầy dung lượng: xoá bớt bản ghi hoặc tệp để ghi và tải lên tiếp.",
			),
		).toBeVisible();
		// No minutes for an account that cannot transcribe.
		expect(screen.queryByText("Chép lời tháng này")).toBeNull();
	});

	it("shows the plus plan with its end date and no daily row", async () => {
		renderCard(
			{
				...FREE,
				plan: {
					name: "plus",
					until: "2026-12-31T16:59:59Z",
					quotaBytes: 5 * 1024 * MB,
					maxFileBytes: 2 * 1024 * MB,
					retentionDays: 14,
				},
				transcription: {
					plan: "plus",
					usedMinutes: 10,
					limitMinutes: 3000,
					resetsAt: "2026-10-31T17:00:00Z",
					capReached: false,
					daily: null,
				},
			},
			1.5 * 1024 * MB,
			5 * 1024 * MB,
		);
		const card = await screen.findByRole("region", { name: "Gói và mức dùng" });
		expect(within(card).getByText("Gói Plus")).toBeVisible();
		expect(card).toHaveTextContent("đến 31/12/2026");
		expect(card).toHaveTextContent("1.5 GB / 5 GB");
		expect(card).toHaveTextContent("Tệp tự xoá sau 14 ngày");
		expect(screen.queryByText("Hôm nay")).toBeNull();
	});

	it("turns the bars warning from 75% and error from 90%", () => {
		expect(usageTone(74)).toBe("primary");
		expect(usageTone(75)).toBe("warning");
		expect(usageTone(90)).toBe("error");
	});

	it("shows uses that run on the user's own keys instead of Scribe's minutes", async () => {
		renderCard(
			{
				...FREE,
				ownKeys: {
					available: true,
					transcribe: "elevenlabs",
					live: "elevenlabs",
					text: "anthropic",
					textModel: "claude-haiku-5-5",
					minutes: 37,
				},
			},
			50 * MB,
			200 * MB,
		);
		const card = await screen.findByRole("region", { name: "Gói và mức dùng" });
		expect(card).toHaveTextContent("Chép lờiDùng key của bạn (ElevenLabs)");
		expect(card).toHaveTextContent("37 phút bằng key của bạn tháng này");
		expect(card).toHaveTextContent(
			"Biên bản, hỏi đáp, dịchDùng key của bạn (Anthropic Claude)",
		);
		expect(screen.queryByText("Chép lời tháng này")).toBeNull();
		expect(screen.queryByText("Hôm nay")).toBeNull();
	});

	it("on a tenant's domain says the owner's AI key is used and keeps the storage", async () => {
		Object.assign(tenant, { domain: "acme.vn", primary: false });
		renderCard(
			{
				...FREE,
				// The backend sends no minutes for runs on the owner's key.
				transcription: { keySource: "tenant" } as CurrentUser["transcription"],
			},
			50 * MB,
			200 * MB,
		);
		const card = await screen.findByRole("region", { name: "Gói và mức dùng" });
		expect(card).toHaveTextContent(
			"Chép lời, biên bản, hỏi đáp, dịchDùng key AI của acme.vn",
		);
		expect(card).toHaveTextContent("Lưu trữ50 MB / 200 MB");
		expect(card).toHaveTextContent("Tệp tự xoá sau 7 ngày");
		expect(screen.queryByText("Chép lời tháng này")).toBeNull();
		expect(screen.queryByText("Hôm nay")).toBeNull();
		expect(card).not.toHaveTextContent("phút đặt lại");
		Object.assign(tenant, { domain: "hoandat.com", primary: true });
	});

	it("names each use the owner has a key for, and the one they have not", async () => {
		Object.assign(tenant, { domain: "acme.vn", primary: false });
		renderCard(
			{
				...FREE,
				transcription: {
					keySource: "tenant",
					plan: "free",
					wholeSession: true,
					tenantKeys: { transcribe: "elevenlabs", text: null },
				},
			},
			50 * MB,
			200 * MB,
		);
		const card = await screen.findByRole("region", { name: "Gói và mức dùng" });
		expect(card).toHaveTextContent("Chép lờiDùng key AI của acme.vn");
		expect(card).toHaveTextContent(
			"Biên bản, hỏi đáp, dịchChủ domain chưa thêm key",
		);
		expect(screen.queryByText("Chép lời tháng này")).toBeNull();
		Object.assign(tenant, { domain: "hoandat.com", primary: true });
	});
});

describe("tenantKeyRows", () => {
	const onTenant = {
		keySource: "tenant",
		plan: "free",
		tenantKeys: { transcribe: "openai", text: "openai" },
	} as const;
	const own = {
		available: true,
		transcribe: null,
		live: null,
		text: "anthropic",
		textModel: null,
		minutes: 0,
	};

	it("leaves out uses on the user's own key and is empty off a tenant", () => {
		expect(tenantKeyRows(null, own)).toEqual([]);
		expect(tenantKeyRows(onTenant, own).map((r) => r.id)).toEqual([
			"usage-tenant-transcribe",
		]);
		expect(tenantKeyRows(onTenant, null).map((r) => r.id)).toEqual([
			"usage-tenant-transcribe",
			"usage-tenant-text",
		]);
	});

	it("falls back to one row when the owner's keys are unknown", () => {
		const unknown = { ...onTenant, tenantKeys: null };
		expect(tenantKeyRows(unknown, null)).toEqual([
			{
				id: "usage-tenant-key",
				label: "ui.usage.tenantKey.label",
				missing: false,
			},
		]);
		expect(tenantKeyRows(unknown, { ...own, transcribe: "openai" })).toEqual(
			[],
		);
	});
});
