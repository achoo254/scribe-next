import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { confirmMatches } from "../features/account/account-api";
import { fakeFetch, jsonResponse } from "../test/fake-fetch";
import { renderWithProviders } from "../test/render-with-providers";
import { DeleteAccountDialog } from "./delete-account-dialog";

const auth = vi.hoisted(() => ({
	forgetDeletedAccount: vi.fn<(deleted: string) => Promise<void>>(
		async () => undefined,
	),
}));
vi.mock("../auth/auth-context", () => ({
	useAuth: () => ({ forgetDeletedAccount: auth.forgetDeletedAccount }),
}));

function open(routes: Parameters<typeof fakeFetch>[0], busy = false) {
	const fake = fakeFetch(routes);
	vi.stubGlobal("fetch", fake.fetcher);
	const view = renderWithProviders(
		<DeleteAccountDialog
			open
			onOpenChange={() => undefined}
			email="minh.anh@example.com"
			busy={busy}
		/>,
	);
	return { ...fake, ...view };
}

afterEach(() => {
	vi.unstubAllGlobals();
	auth.forgetDeletedAccount.mockClear();
});

describe("DeleteAccountDialog", () => {
	it("says what is deleted, that the hoandat account stays, and where to delete it", async () => {
		open({});
		const dialog = await screen.findByRole("dialog", {
			name: "Xoá tài khoản Scribe",
		});
		expect(dialog).toHaveTextContent(
			"Xoá vĩnh viễn tài khoản Scribe của minh.anh@example.com?",
		);
		const scope = within(dialog)
			.getAllByRole("listitem")
			.map((li) => li.textContent);
		expect(scope).toEqual([
			"Mọi bản ghi, tệp tải lên và đoạn cắt.",
			"Lời chép, câu đã sửa, tên người nói, biên bản, dấu mốc, bản dịch và chỉ mục tìm kiếm.",
			"Key riêng và liên kết chat.hoandat.com.",
			"Phiên đăng nhập trên mọi thiết bị và số liệu mức dùng của tài khoản.",
		]);
		expect(dialog).toHaveTextContent("Không hoàn tác được.");
		expect(dialog).toHaveTextContent("Tài khoản hoandat dùng chung");
		const idp = within(dialog).getByRole("link", {
			name: "auth.hoandat.com/account",
		});
		expect(idp).toHaveAttribute("href", "https://auth.hoandat.com/account");
		expect(idp).toHaveAttribute("target", "_blank");
	});

	it("deletes only after the user types XOÁ, then signs this device out", async () => {
		const { calls } = open({
			"DELETE /api/me": jsonResponse({
				deleted: true,
				files: 3,
				bytes: 1024,
				complete: true,
			}),
		});
		const dialog = await screen.findByRole("dialog");
		const confirm = within(dialog).getByRole("button", {
			name: "Xoá tài khoản",
		});
		expect(confirm).toBeDisabled();
		const field = within(dialog).getByLabelText("Gõ XOÁ để xác nhận");
		await userEvent.type(field, "XO");
		expect(confirm).toBeDisabled();
		await userEvent.clear(field);
		await userEvent.type(field, "xóa");
		expect(confirm).toBeEnabled();
		await userEvent.click(confirm);
		await waitFor(() =>
			expect(auth.forgetDeletedAccount).toHaveBeenCalledWith("done"),
		);
		expect(auth.forgetDeletedAccount).toHaveBeenCalledTimes(1);
		const call = calls.find((c) => c.method === "DELETE");
		expect(call?.url).toMatch(/\/api\/me$/);
		expect(call?.body).toEqual({ confirm: "xóa" });
	});

	it("signs out with 'pending' when the server still finishes the deletion", async () => {
		open({
			"DELETE /api/me": jsonResponse({
				deleted: true,
				files: 0,
				bytes: 0,
				complete: false,
			}),
		});
		const dialog = await screen.findByRole("dialog");
		await userEvent.type(within(dialog).getByRole("textbox"), "XOA");
		await userEvent.click(
			within(dialog).getByRole("button", { name: "Xoá tài khoản" }),
		);
		await waitFor(() =>
			expect(auth.forgetDeletedAccount).toHaveBeenCalledWith("pending"),
		);
	});

	it("keeps the account and explains why while a recording runs on the server", async () => {
		open({
			"DELETE /api/me": jsonResponse(
				{ error: "Đang có bản ghi chạy.", code: "RECORDING_OPEN" },
				409,
			),
		});
		const dialog = await screen.findByRole("dialog");
		await userEvent.type(within(dialog).getByRole("textbox"), "XOÁ");
		await userEvent.click(
			within(dialog).getByRole("button", { name: "Xoá tài khoản" }),
		);
		await waitFor(() =>
			expect(within(dialog).getByRole("alert")).toHaveTextContent(
				"Đang có bản ghi chạy. Dừng ghi rồi thử lại.",
			),
		);
		expect(auth.forgetDeletedAccount).not.toHaveBeenCalled();
		expect(
			within(dialog).getByRole("button", { name: "Xoá tài khoản" }),
		).toBeEnabled();
	});

	it("waits while this device records or uploads", async () => {
		open({}, true);
		const dialog = await screen.findByRole("dialog");
		expect(dialog).toHaveTextContent(
			"Dừng ghi và chờ tải xong rồi mới xoá tài khoản.",
		);
		expect(within(dialog).getByRole("textbox")).toBeDisabled();
		expect(
			within(dialog).getByRole("button", { name: "Xoá tài khoản" }),
		).toBeDisabled();
	});
});

describe("confirmMatches", () => {
	it("accepts XOÁ however it is typed, nothing else", () => {
		for (const ok of ["XOÁ", "xoá", "Xóa", "xoa", " XOA "]) {
			expect(confirmMatches(ok), ok).toBe(true);
		}
		for (const bad of ["", "xo", "xoá tài khoản", "delete"]) {
			expect(confirmMatches(bad), bad).toBe(false);
		}
	});
});
