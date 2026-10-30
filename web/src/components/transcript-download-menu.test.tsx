import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { NATIVE_API_ORIGIN } from "../lib/api";
import { renderWithProviders } from "../test/render-with-providers";
import { TranscriptDownloadMenu } from "./transcript-download-menu";

const { share } = vi.hoisted(() => ({
	share: vi.fn(async () => undefined),
}));

vi.mock("../lib/native", () => ({ NativeFiles: { share, cache: vi.fn() } }));

/** The iOS shell: downloads go through the native plugin, observable here. */
function asNative(): void {
	Reflect.set(globalThis, "Capacitor", { isNativePlatform: () => true });
}

afterEach(() => {
	Reflect.deleteProperty(globalThis, "Capacitor");
	vi.clearAllMocks();
});

describe("TranscriptDownloadMenu", () => {
	it("lists every export format under one “Tải xuống” button", async () => {
		renderWithProviders(
			<TranscriptDownloadMenu
				scope={{ fileIds: ["a"] }}
				name="Họp"
				data-id="dl"
			/>,
		);
		expect(screen.queryByRole("menu")).toBeNull();
		await userEvent.click(
			await screen.findByRole("button", { name: "Tải xuống" }),
		);
		const items = await screen.findAllByRole("menuitem");
		expect(items.map((item) => item.textContent)).toEqual([
			"TXT",
			"SRT",
			"VTT",
			"Markdown",
			"Word (.docx)",
		]);
	});

	it("downloads the chosen format of the whole session through the app", async () => {
		asNative();
		renderWithProviders(
			<TranscriptDownloadMenu
				scope={{ session: "aaaaaaaa-1111" }}
				name="Họp đầu tuần"
				data-id="dl"
			/>,
		);
		await userEvent.click(
			await screen.findByRole("button", { name: "Tải xuống" }),
		);
		await userEvent.click(
			await screen.findByRole("menuitem", { name: "Word (.docx)" }),
		);
		const query = new URLSearchParams({
			session: "aaaaaaaa-1111",
			format: "docx",
			name: "Họp đầu tuần",
		});
		expect(share).toHaveBeenCalledWith({
			url: `${NATIVE_API_ORIGIN}/api/transcripts/export?${query.toString()}`,
		});
		expect(screen.queryByRole("menu")).toBeNull();
	});

	it("offers no language without a translation", async () => {
		renderWithProviders(
			<TranscriptDownloadMenu
				scope={{ fileIds: ["a"] }}
				name="Họp"
				data-id="dl"
			/>,
		);
		await userEvent.click(
			await screen.findByRole("button", { name: "Tải xuống" }),
		);
		await screen.findAllByRole("menuitem");
		expect(screen.queryAllByRole("menuitemradio")).toEqual([]);
	});

	it("downloads the chosen translation in any format", async () => {
		asNative();
		renderWithProviders(
			<TranscriptDownloadMenu
				scope={{ session: "aaaaaaaa-1111" }}
				name="Họp đầu tuần"
				data-id="dl"
				languages={["zh-CN", "vi", "en"]}
			/>,
		);
		await userEvent.click(
			await screen.findByRole("button", { name: "Tải xuống" }),
		);
		const languages = await screen.findAllByRole("menuitemradio");
		expect(languages.map((item) => item.textContent)).toEqual([
			"Gốc",
			"Tiếng Anh",
			"Tiếng Việt",
			"Tiếng Trung (giản thể)",
		]);
		expect(languages[0]).toHaveAttribute("aria-checked", "true");
		await userEvent.click(languages[1] as HTMLElement);
		// Picking the language keeps the menu open for the format.
		expect(
			await screen.findByRole("menuitemradio", { name: "Tiếng Anh" }),
		).toHaveAttribute("aria-checked", "true");
		await userEvent.click(screen.getByRole("menuitem", { name: "SRT" }));
		const query = new URLSearchParams({
			session: "aaaaaaaa-1111",
			format: "srt",
			name: "Họp đầu tuần",
			lang: "en",
		});
		expect(share).toHaveBeenCalledWith({
			url: `${NATIVE_API_ORIGIN}/api/transcripts/export?${query.toString()}`,
		});
	});
});
