/** Vietnamese messages for recording failures, with where to fix them. */

/** Recording sources, each switched on or off by the user (≥ 1 on). */
export interface RecordSources {
	mic: boolean;
	systemAudio: boolean;
	screen: boolean;
}

export interface BrowserInfo {
	userAgent: string;
	maxTouchPoints: number;
}

function currentBrowser(): BrowserInfo {
	if (typeof navigator === "undefined") {
		return { userAgent: "", maxTouchPoints: 0 };
	}
	return {
		userAgent: navigator.userAgent,
		maxTouchPoints: navigator.maxTouchPoints ?? 0,
	};
}

/**
 * Where to allow the microphone, by browser. A web page cannot open the
 * system settings, only point to them: Edge/Chrome/Firefox on iOS keep the
 * permission in iOS Settings, Safari in "aA" → website settings, desktop
 * and Android in the permission icon of the address bar.
 */
export function micPermissionHint(
	browser: BrowserInfo = currentBrowser(),
): string {
	const ua = browser.userAgent;
	const isIOS =
		/iPad|iPhone|iPod/.test(ua) ||
		(ua.includes("Macintosh") && browser.maxTouchPoints > 1);
	if (isIOS) {
		if (ua.includes("EdgiOS")) {
			return "Vào Cài đặt iOS → Edge → Micro → bật, rồi tải lại trang và thử lại.";
		}
		if (ua.includes("CriOS")) {
			return "Vào Cài đặt iOS → Chrome → Micro → bật, rồi tải lại trang và thử lại.";
		}
		if (ua.includes("FxiOS")) {
			return "Vào Cài đặt iOS → Firefox → Micro → bật, rồi tải lại trang và thử lại.";
		}
		return "Bấm nút “aA” bên trái thanh địa chỉ → Cài đặt Trang web → Micro → Cho phép (hoặc Cài đặt iOS → Safari → Micro), rồi tải lại trang.";
	}
	return "Bấm biểu tượng khoá/quyền bên trái thanh địa chỉ → cho phép Micro, rồi tải lại trang và thử lại.";
}

/**
 * Message for a failed start. getUserMedia/getDisplayMedia reject with a
 * named DOMException; NotAllowedError (permission denied or picker
 * cancelled) is the common case on iOS WebKit. Other errors (backend,
 * network) keep their own message.
 */
export function startErrorMessage(
	error: unknown,
	sources: RecordSources,
	browser: BrowserInfo = currentBrowser(),
): string {
	if (error instanceof DOMException) {
		switch (error.name) {
			case "NotAllowedError":
				return sources.screen || sources.systemAudio
					? "Chưa được cấp quyền chia sẻ màn hình / micro — hãy chọn nguồn chia sẻ và cho phép truy cập, rồi thử lại."
					: `Trình duyệt chưa được cấp quyền Micro. ${micPermissionHint(browser)}`;
			case "NotFoundError":
			case "OverconstrainedError":
				return "Không tìm thấy thiết bị micro phù hợp.";
			case "NotReadableError":
				return "Micro đang được ứng dụng khác sử dụng — hãy đóng ứng dụng đó rồi thử lại.";
			case "AbortError":
				return "Không truy cập được micro (thiết bị bận hoặc bị gián đoạn) — thử lại.";
			case "SecurityError":
				return "Ghi âm cần kết nối bảo mật (HTTPS).";
			default:
				return error.message || "Không bắt đầu ghi được.";
		}
	}
	return error instanceof Error && error.message
		? error.message
		: "Không bắt đầu ghi được (quyền truy cập bị từ chối?)";
}

export const INTERRUPT_MIC_ENDED =
	"Bản ghi đã dừng (mất micro — có thể do màn hình bị khóa) — đã lưu phần ghi được.";
export const INTERRUPT_HIDDEN =
	"Bản ghi đã dừng khi màn hình bị khóa / chuyển ứng dụng — đã lưu phần ghi được.";
export const CHUNKS_LOST =
	"Một số đoạn bị lỗi mạng — đã lưu phần tải được (bản ghi có thể bị cắt ngắn).";
export const NO_SOURCE =
	"Không lấy được nguồn ghi nào — kiểm tra quyền mic / chia sẻ màn hình.";
