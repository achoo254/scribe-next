import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { CurrentUser } from "../auth/auth-api";
import { fakeFetch, jsonResponse } from "../test/fake-fetch";
import { NO_OWN_KEYS as NO_KEYS } from "../test/own-keys";
import { renderWithProviders } from "../test/render-with-providers";
import { UsageCard, usageTone } from "./usage-card";

const auth = vi.hoisted(() => ({ user: null as CurrentUser | null }));
vi.mock("../auth/auth-context", () => ({
	useAuth: () => ({ user: auth.user, refresh: async () => undefined }),
}));

const MB = 1024 * 1024;

const FREE: CurrentUser = {
	email: "a@b.c",
	canTranscribe: true,
	plan: {
		quotaBytes: 200 * MB,
		maxFileBytes: 200 * MB,
		retentionDays: 7,
	},
	transcription: null,
	ownKeys: NO_KEYS,
};

function renderCard(user: CurrentUser, usageBytes: number, quotaBytes: number) {
	auth.user = user;
	const fake = fakeFetch({
		"GET /api/me/usage": jsonResponse({ usageBytes, quotaBytes }),
		"GET /api/me/keys": jsonResponse({
			available: true,
			keys: [],
			prefs: { transcribe: null, live: null, text: null, textModel: null },
			active: { transcribe: null, live: null, text: null, textModel: null },
			models: {},
		}),
	});
	vi.stubGlobal("fetch", fake.fetcher);
	renderWithProviders(<UsageCard />);
	return fake;
}

afterEach(() => {
	vi.unstubAllGlobals();
});

describe("UsageCard", () => {
	it("shows how long files are kept and storage, with no plan and no AI minutes", async () => {
		renderCard(FREE, 50 * MB, 200 * MB);
		const card = await screen.findByRole("region", { name: "Mức dùng" });
		expect(card).not.toHaveTextContent(/Gói|Free|Plus/);
		expect(card).toHaveTextContent("Tệp tự xoá sau 7 ngày");
		expect(card).toHaveTextContent("Lưu trữ50 MB / 200 MB");
		expect(card).not.toHaveTextContent(/phút/);
		expect(screen.queryByText(/Đã đầy dung lượng/)).toBeNull();
	});

	it("says storage is full when the plan's limit is reached", async () => {
		renderCard(FREE, 230 * MB, 200 * MB);
		expect(
			await screen.findByText(
				"Đã đầy dung lượng: xoá bớt bản ghi hoặc tệp để ghi và tải lên tiếp.",
			),
		).toBeVisible();
	});

	it("turns the bars warning from 75% and error from 90%", () => {
		expect(usageTone(74)).toBe("primary");
		expect(usageTone(75)).toBe("warning");
		expect(usageTone(90)).toBe("error");
	});

	it("shows which own key each AI use runs on, and no key hint", async () => {
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
		const card = await screen.findByRole("region", { name: "Mức dùng" });
		expect(card).toHaveTextContent("Chép lờiDùng key của bạn (ElevenLabs)");
		expect(card).toHaveTextContent("37 phút bằng key của bạn tháng này");
		expect(card).toHaveTextContent(
			"Biên bản, hỏi đáp, dịchDùng key của bạn (Anthropic Claude)",
		);
		expect(within(card).queryByRole("button", { name: "Mở Key riêng" })).toBe(
			null,
		);
	});

	it("says AI needs the user's key and opens Key riêng when there is none", async () => {
		const fake = renderCard(FREE, 50 * MB, 200 * MB);
		const card = await screen.findByRole("region", { name: "Mức dùng" });
		expect(card).toHaveTextContent(
			"Chép lời, biên bản, hỏi đáp và dịch chạy bằng API key của bạn",
		);
		await userEvent.click(
			within(card).getByRole("button", { name: "Mở Key riêng" }),
		);
		expect(
			await screen.findByRole("dialog", { name: "Key riêng" }),
		).toBeVisible();
		await waitFor(() =>
			expect(fake.calls.map((call) => call.url)).toContain("/api/me/keys"),
		);
	});

	it("names only the missing use when one AI use has a key", async () => {
		renderCard(
			{ ...FREE, ownKeys: { ...NO_KEYS, transcribe: "openai" } },
			50 * MB,
			200 * MB,
		);
		const card = await screen.findByRole("region", { name: "Mức dùng" });
		expect(card).toHaveTextContent(
			"Biên bản, hỏi đáp và dịch chạy bằng API key của bạn.",
		);
		expect(card).not.toHaveTextContent("Chép lời chạy bằng API key");
	});

	it("treats every use as missing when the server cannot store keys", async () => {
		renderCard(
			{
				...FREE,
				ownKeys: { ...NO_KEYS, available: false, transcribe: "openai" },
			},
			50 * MB,
			200 * MB,
		);
		const card = await screen.findByRole("region", { name: "Mức dùng" });
		expect(card).not.toHaveTextContent("Dùng key của bạn");
		expect(card).toHaveTextContent(
			"Chép lời, biên bản, hỏi đáp và dịch chạy bằng API key của bạn",
		);
	});
});
