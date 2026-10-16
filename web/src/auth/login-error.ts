import { errorMessage } from "../lib/api";

/**
 * Codes the backend appends to /login?error=… when a web sign-in through
 * hoandat-auth does not complete (`auth.rs`).
 */
const LOGIN_ERRORS: Record<string, string> = {
	denied: "Bạn đã từ chối đăng nhập ở trang tài khoản.",
	expired: "Lượt đăng nhập đã hết hạn — bấm Đăng nhập lại.",
	invalid: "Phản hồi đăng nhập không hợp lệ — bấm Đăng nhập lại.",
	unverified:
		"Email của tài khoản chưa được xác minh — đăng nhập bằng Google hoặc GitHub đã xác minh email.",
	forbidden: "Tài khoản này chưa được phép dùng Scribe.",
	idp: "Máy chủ đăng nhập báo lỗi — thử lại sau.",
	failed: "Đăng nhập thất bại — thử lại sau.",
};

/** Text for an `?error=` code of /login; empty when there is none. */
export function loginErrorFromCode(code: string | null): string {
	if (!code) return "";
	return LOGIN_ERRORS[code] ?? LOGIN_ERRORS.failed ?? "";
}

/** Text for an error thrown by the iOS sign-in sheet or the backend (already Vietnamese). */
export function loginErrorMessage(error: unknown): string {
	return errorMessage(error, "Đăng nhập thất bại");
}
