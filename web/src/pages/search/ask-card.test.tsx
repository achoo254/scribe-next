import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { CurrentUser } from "../../auth/auth-api";
import type { AskReply } from "../../features/search/search-api";
import { fakeFetch, jsonResponse } from "../../test/fake-fetch";
import { ALL_OWN_KEYS, NO_OWN_KEYS } from "../../test/own-keys";
import { renderWithProviders } from "../../test/render-with-providers";
import { AskCard } from "./ask-card";

const auth = vi.hoisted(() => ({ user: null as CurrentUser | null }));
vi.mock("../../auth/auth-context", () => ({
	useAuth: () => ({ user: auth.user, refresh: async () => undefined }),
}));

const REPLY: AskReply = {
	answer: "Khách chốt 5 triệu [1].",
	citations: [
		{
			n: 1,
			key: "s:aaaaaaaa-1111",
			title: "Họp khách",
			createdAt: null,
			at: 75,
			text: "Chốt giá 5 triệu.",
		},
	],
};

const USER: CurrentUser = {
	email: "a@b.c",
	canTranscribe: true,
	ownKeys: ALL_OWN_KEYS,
};

beforeEach(() => {
	auth.user = USER;
});

afterEach(() => {
	vi.unstubAllGlobals();
});

function renderCard(reply: Response, transcriptKey?: string) {
	const fake = fakeFetch({ "POST /api/ask": reply });
	vi.stubGlobal("fetch", fake.fetcher);
	const onCite = vi.fn();
	renderWithProviders(
		<AskCard transcriptKey={transcriptKey} onCite={onCite} />,
	);
	return { ...fake, onCite };
}

async function ask(question: string): Promise<void> {
	await userEvent.type(
		screen.getByRole("textbox", { name: "Câu hỏi cho Scribe" }),
		question,
	);
	await userEvent.click(screen.getByRole("button", { name: "Hỏi" }));
}

describe("AskCard", () => {
	it("answers with sources that play the transcript from their second", async () => {
		const { calls, onCite } = renderCard(
			jsonResponse(REPLY),
			"s:aaaaaaaa-1111",
		);
		expect(screen.getByText(/Hỏi về bản ghi này/)).toBeInTheDocument();
		await ask("  Giá chốt bao nhiêu? ");
		expect(
			screen.getByText("Bạn hỏi: Giá chốt bao nhiêu?"),
		).toBeInTheDocument();
		const chip = await screen.findByRole("button", {
			name: "Nguồn 1: Họp khách, nghe từ 1:15",
		});
		expect(chip).toHaveTextContent("1");
		expect(calls[0]?.body).toEqual({
			question: "Giá chốt bao nhiêu?",
			key: "s:aaaaaaaa-1111",
			lang: "vi",
		});
		await userEvent.click(chip);
		expect(onCite).toHaveBeenCalledWith(REPLY.citations[0]);
		// The source list repeats it with the sentence.
		expect(screen.getByText("Chốt giá 5 triệu.")).toBeInTheDocument();
		// Asked questions stay; the field is empty for the next one.
		expect(
			screen.getByRole("textbox", { name: "Câu hỏi cho Scribe" }),
		).toHaveValue("");
	});

	it("shows the server's NO_USER_KEY refusal with a way to Key riêng", async () => {
		renderCard(
			jsonResponse(
				{
					error: "Hỏi đáp cần API key của bạn. Thêm key trong Key riêng.",
					code: "NO_USER_KEY",
				},
				403,
			),
		);
		expect(screen.getByText(/trên mọi bản ghi của bạn/)).toBeInTheDocument();
		await ask("Ai gửi báo giá?");
		expect(
			await screen.findByText(
				"Hỏi đáp cần API key của bạn. Thêm key trong Key riêng.",
			),
		).toBeInTheDocument();
		expect(screen.getByRole("button", { name: "Mở Key riêng" })).toBeVisible();
	});

	it("explains that asking needs the user's text key, without sending", async () => {
		auth.user = { ...USER, ownKeys: NO_OWN_KEYS };
		const { calls } = renderCard(jsonResponse(REPLY));
		expect(
			screen.getByText(
				"Biên bản, hỏi đáp và dịch chạy bằng API key của bạn. Thêm key OpenAI, Google Gemini hoặc Anthropic Claude trong Key riêng.",
			),
		).toBeVisible();
		await userEvent.type(
			screen.getByRole("textbox", { name: "Câu hỏi cho Scribe" }),
			"Ai gửi báo giá?",
		);
		expect(screen.getByRole("button", { name: "Hỏi" })).toBeDisabled();
		await userEvent.click(screen.getByRole("button", { name: "Mở Key riêng" }));
		expect(
			await screen.findByRole("dialog", { name: "Key riêng" }),
		).toBeVisible();
		expect(calls.some((call) => call.url === "/api/ask")).toBe(false);
	});

	it("shows the wait while Scribe answers and blocks a second question", async () => {
		let release: (value: Response) => void = () => undefined;
		const pending = new Promise<Response>((resolve) => {
			release = resolve;
		});
		const fetcher = vi.fn(() => pending);
		vi.stubGlobal("fetch", fetcher);
		renderWithProviders(<AskCard onCite={() => undefined} />);
		await ask("Giá?");
		expect(await screen.findByText(/Đang tìm câu trả lời/)).toBeInTheDocument();
		await userEvent.type(
			screen.getByRole("textbox", { name: "Câu hỏi cho Scribe" }),
			"Hạn?",
		);
		await userEvent.click(screen.getByRole("button", { name: /Hỏi/ }));
		expect(fetcher).toHaveBeenCalledTimes(1);
		release(
			jsonResponse({ answer: "Không tìm thấy trong bản ghi.", citations: [] }),
		);
		await waitFor(() =>
			expect(
				screen.getByText("Không tìm thấy trong bản ghi."),
			).toBeInTheDocument(),
		);
	});
});
