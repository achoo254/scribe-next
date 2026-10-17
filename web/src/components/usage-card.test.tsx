import { screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { CurrentUser } from "../auth/auth-api";
import { fakeFetch, jsonResponse } from "../test/fake-fetch";
import { renderWithProviders } from "../test/render-with-providers";
import { UsageCard, usageTone } from "./usage-card";

const auth = vi.hoisted(() => ({ user: null as CurrentUser | null }));
vi.mock("../auth/auth-context", () => ({
	useAuth: () => ({ user: auth.user }),
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
});
