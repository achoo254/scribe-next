import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { fakeFetch, jsonResponse } from "../test/fake-fetch";
import { renderWithProviders } from "../test/render-with-providers";
import { UserKeysDialog } from "./user-keys-dialog";

const auth = vi.hoisted(() => ({ refresh: vi.fn(async () => undefined) }));
vi.mock("../auth/auth-context", () => ({
	useAuth: () => ({ refresh: auth.refresh }),
}));

const MODELS = {
	openai: [
		{ id: "gpt-4.1-mini", label: "GPT-4.1 mini" },
		{ id: "gpt-4.1-nano", label: "GPT-4.1 nano (rẻ hơn)" },
	],
	gemini: [{ id: "gemini-3.8-flash", label: "Gemini 3.8 Flash" }],
	anthropic: [
		{ id: "claude-haiku-5-5", label: "Claude Haiku 5.5" },
		{ id: "claude-sonnet-5-5", label: "Claude Sonnet 5.5" },
	],
};

const NO_CHOICE = { transcribe: null, live: null, text: null, textModel: null };

function keysReply(overrides: Record<string, unknown> = {}) {
	return {
		available: true,
		keys: [
			{
				provider: "openai",
				last4: "1234",
				status: "valid",
				updatedAt: "2026-10-19T02:00:00Z",
				lastCheckedAt: "2026-10-19T02:00:00Z",
			},
		],
		prefs: NO_CHOICE,
		active: { ...NO_CHOICE, text: "openai", textModel: "gpt-4.1-mini" },
		models: MODELS,
		...overrides,
	};
}

function open(routes: Parameters<typeof fakeFetch>[0]) {
	const fake = fakeFetch(routes);
	vi.stubGlobal("fetch", fake.fetcher);
	renderWithProviders(<UserKeysDialog open onOpenChange={() => undefined} />);
	return fake;
}

afterEach(() => {
	vi.unstubAllGlobals();
	auth.refresh.mockClear();
});

describe("UserKeysDialog", () => {
	it("lists every provider with what it is for and shows a saved key only by its last four characters", async () => {
		open({ "GET /api/me/keys": jsonResponse(keysReply()) });
		const dialog = await screen.findByRole("dialog", { name: "Key riêng" });
		const openai = await within(dialog).findByRole("region", {
			name: "OpenAI",
		});
		expect(openai).toHaveTextContent("••••1234 · đã kiểm 19/10");
		expect(
			within(dialog).getByRole("region", { name: "ElevenLabs" }),
		).toHaveTextContent("Chưa có key");
		expect(
			within(dialog).getByRole("region", { name: "Google Gemini" }),
		).toHaveTextContent("mốc giờ của câu là ước lượng");
		expect(
			within(dialog).getByRole("region", { name: "Anthropic Claude" }),
		).toHaveTextContent("Claude không chép lời.");
		const field = within(openai).getByLabelText("Key OpenAI");
		expect(field).toHaveAttribute("type", "password");
		expect(field).toHaveValue("");
		expect(dialog).toHaveTextContent("được mã hoá và lưu trên máy chủ Scribe");
		expect(
			within(dialog).getByRole("link", { name: /Tạo key ở ElevenLabs/ }),
		).toHaveAttribute("href", "https://elevenlabs.io/app/settings/api-keys");
	});

	it("checks and saves a key, then clears the field and refreshes the usage card", async () => {
		const user = userEvent.setup();
		const saved = {
			provider: "elevenlabs",
			last4: "wxyz",
			status: "valid",
			updatedAt: "2026-10-19T03:00:00Z",
			lastCheckedAt: "2026-10-19T03:00:00Z",
		};
		const fake = open({
			"GET /api/me/keys": jsonResponse(keysReply()),
			"PUT /api/me/keys/elevenlabs": jsonResponse(saved),
		});
		const section = await screen.findByRole("region", { name: "ElevenLabs" });
		const field = within(section).getByLabelText("Key ElevenLabs");
		await user.type(field, "  sk_test_fake_key_wxyz  ");
		await user.click(
			within(section).getByRole("button", { name: "Lưu và kiểm" }),
		);
		await waitFor(() => expect(field).toHaveValue(""));
		const put = fake.calls.find((c) => c.method === "PUT");
		expect(put?.url).toBe("/api/me/keys/elevenlabs");
		expect(put?.body).toEqual({ key: "sk_test_fake_key_wxyz" });
		expect(
			await screen.findByText("Đã kiểm và lưu key ElevenLabs"),
		).toBeVisible();
		expect(auth.refresh).toHaveBeenCalled();
	});

	it("shows why the provider refused a key and keeps what was typed", async () => {
		const user = userEvent.setup();
		open({
			"GET /api/me/keys": jsonResponse(keysReply()),
			"PUT /api/me/keys/gemini": jsonResponse(
				{
					error:
						"Google Gemini từ chối key này: key sai, đã bị thu hồi hoặc thiếu quyền.",
					code: "KEY_REJECTED",
				},
				400,
			),
		});
		const section = await screen.findByRole("region", {
			name: "Google Gemini",
		});
		const field = within(section).getByLabelText("Key Google Gemini");
		await user.type(field, "gm-test-wrong-key-000000");
		await user.click(
			within(section).getByRole("button", { name: "Lưu và kiểm" }),
		);
		expect(await within(section).findByRole("alert")).toHaveTextContent(
			"Google Gemini từ chối key này",
		);
		expect(field).toHaveValue("gm-test-wrong-key-000000");
	});

	it("deletes a saved key and warns about a key the provider refused at use time", async () => {
		const user = userEvent.setup();
		const fake = open({
			"GET /api/me/keys": [
				jsonResponse(
					keysReply({
						keys: [
							{
								provider: "openai",
								last4: "1234",
								status: "quota",
								updatedAt: "2026-10-19T02:00:00Z",
								lastCheckedAt: "2026-10-19T04:00:00Z",
							},
						],
						active: NO_CHOICE,
					}),
				),
				jsonResponse(keysReply({ keys: [], active: NO_CHOICE })),
			],
			"DELETE /api/me/keys/openai": jsonResponse({ deleted: true }),
		});
		const section = await screen.findByRole("region", { name: "OpenAI" });
		expect(within(section).getByRole("alert")).toHaveTextContent(
			"Tài khoản của key đã hết credit",
		);
		await user.click(within(section).getByRole("button", { name: "Xoá key" }));
		await waitFor(() =>
			expect(fake.calls.some((c) => c.method === "DELETE")).toBe(true),
		);
		expect(await screen.findByText("Đã xoá key OpenAI")).toBeVisible();
		expect(within(section).getByText("Chưa có key")).toBeVisible();
	});

	it("says when the server cannot store keys and turns saving off", async () => {
		open({
			"GET /api/me/keys": jsonResponse(
				keysReply({ available: false, keys: [] }),
			),
		});
		expect(await screen.findByText(/Máy chủ chưa bật Key riêng/)).toBeVisible();
		for (const field of screen.getAllByLabelText(
			/^Key (ElevenLabs|OpenAI|Google Gemini|Anthropic Claude)$/,
		)) {
			expect(field).toBeDisabled();
		}
		expect(screen.queryByRole("group", { name: "Chép lời tệp" })).toBeNull();
	});

	it("offers each use only the providers with a valid key and saves the choice", async () => {
		const user = userEvent.setup();
		const withClaude = keysReply({
			keys: [
				{
					provider: "openai",
					last4: "1234",
					status: "valid",
					updatedAt: null,
					lastCheckedAt: null,
				},
				{
					provider: "anthropic",
					last4: "abcd",
					status: "valid",
					updatedAt: null,
					lastCheckedAt: null,
				},
			],
			active: {
				transcribe: "openai",
				live: "openai",
				text: "openai",
				textModel: "gpt-4.1-mini",
			},
		});
		const fake = open({
			"GET /api/me/keys": jsonResponse(withClaude),
			"PUT /api/me/key-prefs": jsonResponse({
				...withClaude,
				prefs: { ...NO_CHOICE, text: "anthropic" },
				active: {
					...withClaude.active,
					text: "anthropic",
					textModel: "claude-haiku-5-5",
				},
			}),
		});
		const text = await screen.findByRole("group", {
			name: "Biên bản, hỏi đáp, dịch",
		});
		expect(within(text).getByRole("radio", { name: "OpenAI" })).toBeChecked();
		expect(
			within(text).getByRole("radio", { name: "Google Gemini (chưa có key)" }),
		).toBeDisabled();
		const transcribe = screen.getByRole("group", { name: "Chép lời tệp" });
		expect(
			within(transcribe).queryByRole("radio", { name: /Claude/ }),
		).toBeNull();
		const live = screen.getByRole("group", {
			name: "Chép trực tiếp trên iPhone",
		});
		expect(within(live).queryByRole("radio", { name: /Gemini/ })).toBeNull();

		await user.click(
			within(text).getByRole("radio", { name: "Anthropic Claude" }),
		);
		await waitFor(() =>
			expect(
				within(text).getByRole("radio", { name: "Anthropic Claude" }),
			).toBeChecked(),
		);
		const put = fake.calls.find((c) => c.url === "/api/me/key-prefs");
		expect(put?.body).toEqual({ ...NO_CHOICE, text: "anthropic" });
		expect(within(text).getByRole("combobox", { name: "Model" })).toHaveValue(
			"claude-haiku-5-5",
		);
	});

	it("says a use runs on Scribe's key when no key of the user can do it", async () => {
		open({ "GET /api/me/keys": jsonResponse(keysReply()) });
		const live = await screen.findByRole("group", {
			name: "Chép trực tiếp trên iPhone",
		});
		expect(live).toHaveTextContent("Đang dùng key của Scribe");
		expect(
			within(live).getByRole("radio", { name: "OpenAI" }),
		).not.toBeChecked();
	});
});
