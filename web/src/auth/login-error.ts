import { errorMessage } from "../lib/api";

/**
 * Firebase popup errors carry an English message with a `code`
 * (`auth/popup-blocked`…); the common ones get a Vietnamese text. Errors
 * from the backend already are Vietnamese and pass through.
 */
const FIREBASE_MESSAGES: Record<string, string> = {
	"auth/popup-closed-by-user": "Cửa sổ đăng nhập đã đóng trước khi hoàn tất.",
	"auth/cancelled-popup-request":
		"Cửa sổ đăng nhập đã đóng trước khi hoàn tất.",
	"auth/popup-blocked":
		"Trình duyệt đã chặn cửa sổ đăng nhập — cho phép cửa sổ bật lên rồi thử lại.",
	"auth/network-request-failed":
		"Không kết nối được Google — kiểm tra mạng rồi thử lại.",
	"auth/unauthorized-domain": "Tên miền này chưa được phép đăng nhập Google.",
};

export function loginErrorMessage(error: unknown): string {
	if (typeof error === "object" && error !== null && "code" in error) {
		const code = error.code;
		if (typeof code === "string" && code in FIREBASE_MESSAGES) {
			return FIREBASE_MESSAGES[code] ?? "Đăng nhập thất bại";
		}
	}
	return errorMessage(error, "Đăng nhập thất bại");
}
