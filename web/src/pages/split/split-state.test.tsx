import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { fakeFetch, jsonResponse } from "../../test/fake-fetch";
import { renderWithProviders } from "../../test/render-with-providers";
import { SplitPageProvider, useSplitPageState } from "./split-state";

/*
 * A file uploaded on "Cắt file" is only the input of a cut: it is not a
 * recording, so it must not join "Lịch sử ghi" (the record page's upload
 * asks for that with `purpose: "transcribe"`).
 */

afterEach(() => {
	vi.unstubAllGlobals();
});

const audio = () =>
	new File([new Uint8Array(1024)], "phong-van.mp3", { type: "audio/mpeg" });

function ChooseProbe() {
	const { choose, upload } = useSplitPageState();
	return (
		<>
			<button type="button" onClick={() => void choose(audio())}>
				Chọn tệp
			</button>
			{upload.result && <p>Đã tải lên {upload.result.name}</p>}
		</>
	);
}

describe("SplitPageProvider", () => {
	it("uploads the chosen file outside the recording history", async () => {
		const fake = fakeFetch({
			"POST /api/upload/init": jsonResponse({ sessionId: "s1" }),
			"GET /api/upload/status": jsonResponse({ written: 0 }),
			"POST /api/upload/chunk": jsonResponse({ written: 1024 }),
			"POST /api/upload/finalize": jsonResponse({
				fileId: "u1",
				name: "phong-van.mp3",
				durationS: 30,
			}),
		});
		vi.stubGlobal("fetch", fake.fetcher);
		const { queryClient } = renderWithProviders(
			<SplitPageProvider>
				<ChooseProbe />
			</SplitPageProvider>,
		);
		const refresh = vi.spyOn(queryClient, "invalidateQueries");

		await userEvent.click(screen.getByRole("button", { name: "Chọn tệp" }));
		expect(
			await screen.findByText("Đã tải lên phong-van.mp3"),
		).toBeInTheDocument();
		expect(fake.calls[0]?.body).toEqual({ name: "phong-van.mp3", size: 1024 });
		// The quota and the server's file list change, the history does not.
		await waitFor(() =>
			expect(refresh).toHaveBeenCalledWith({ queryKey: ["managed-files"] }),
		);
		expect(refresh).not.toHaveBeenCalledWith({ queryKey: ["recordings"] });
	});
});
