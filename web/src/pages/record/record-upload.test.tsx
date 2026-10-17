import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { fakeFetch, jsonResponse } from "../../test/fake-fetch";
import { renderWithProviders } from "../../test/render-with-providers";
import { RecordUpload } from "./record-upload";
import { RecordUploadProvider } from "./record-upload-state";

afterEach(() => {
	vi.unstubAllGlobals();
});

function renderUpload(routes: Parameters<typeof fakeFetch>[0]) {
	const fake = fakeFetch(routes);
	vi.stubGlobal("fetch", fake.fetcher);
	const view = renderWithProviders(
		<RecordUploadProvider>
			<RecordUpload />
		</RecordUploadProvider>,
	);
	return { ...view, fake };
}

function fileInput(): HTMLInputElement {
	const input = document.querySelector<HTMLInputElement>(
		'[data-id="record-upload-input"]',
	);
	if (!input) throw new Error("no file input");
	return input;
}

const audio = () =>
	new File([new Uint8Array(1024)], "phong-van.mp3", { type: "audio/mpeg" });

describe("RecordUpload", () => {
	it("uploads a chosen audio file in chunks and points to the history", async () => {
		const { fake, queryClient } = renderUpload({
			"POST /api/upload/init": jsonResponse({ sessionId: "s1" }),
			"GET /api/upload/status": jsonResponse({ written: 0 }),
			"POST /api/upload/chunk": jsonResponse({ written: 1024 }),
			"POST /api/upload/finalize": jsonResponse({
				fileId: "u1",
				name: "phong-van.mp3",
				durationS: 30,
			}),
		});
		const refresh = vi.spyOn(queryClient, "invalidateQueries");
		const input = fileInput();
		expect(input).toHaveAttribute("accept", "audio/*,video/*");
		await userEvent.upload(input, audio());

		expect(await screen.findByText(/Đã tải lên “phong-van.mp3”/)).toBeVisible();
		expect(fake.calls.map((c) => `${c.method} ${c.url}`)).toEqual([
			"POST /api/upload/init",
			"GET /api/upload/status?session=s1",
			"POST /api/upload/chunk?session=s1&offset=0",
			"POST /api/upload/finalize",
		]);
		expect(fake.calls[0]?.body).toEqual({
			name: "phong-van.mp3",
			size: 1024,
			purpose: "transcribe",
		});
		expect(refresh).toHaveBeenCalledWith({ queryKey: ["recordings"] });
	});

	it("shows the plan's limit when the backend refuses the file", async () => {
		renderUpload({
			"POST /api/upload/init": jsonResponse(
				{ error: "Mỗi tệp tối đa 200 MB." },
				413,
			),
		});
		await userEvent.upload(fileInput(), audio());
		expect(await screen.findByText("Mỗi tệp tối đa 200 MB.")).toBeVisible();
		expect(screen.getByRole("button", { name: "Tải tiếp" })).toBeVisible();
	});
});
