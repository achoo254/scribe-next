import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { AskReply } from "../../features/search/search-api";
import { fakeFetch, jsonResponse } from "../../test/fake-fetch";
import { renderWithProviders } from "../../test/render-with-providers";
import { AskCard } from "./ask-card";

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

	it("says when a used-up allowance opens again", async () => {
		renderCard(
			jsonResponse(
				{
					error: "Hôm nay đã dùng 30/30 lượt hỏi. Mở lại lúc 0 giờ.",
					code: "ASK_DAILY_LIMIT",
					usedMinutes: 30,
					limitMinutes: 30,
					retryAt: "2026-10-16T17:00:00+00:00",
				},
				403,
			),
		);
		expect(screen.getByText(/trên mọi bản ghi của bạn/)).toBeInTheDocument();
		await ask("Ai gửi báo giá?");
		expect(
			await screen.findByText(
				"Hôm nay đã dùng 30/30 lượt hỏi. Mở lại lúc 00:00 17/10.",
			),
		).toBeInTheDocument();
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
