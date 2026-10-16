/** Recorders (browser and iOS): failures, where to allow the microphone, background notices. */
export const recorder = {
	/** Name of a new browser recording; `{date}` is the local date and time. */
	defaultName: "Bản ghi {date}",
	micHint: {
		iosEdge:
			"Vào Cài đặt iOS → Edge → Micro → bật, rồi tải lại trang và thử lại.",
		iosChrome:
			"Vào Cài đặt iOS → Chrome → Micro → bật, rồi tải lại trang và thử lại.",
		iosFirefox:
			"Vào Cài đặt iOS → Firefox → Micro → bật, rồi tải lại trang và thử lại.",
		iosSafari:
			"Bấm nút “aA” bên trái thanh địa chỉ → Cài đặt Trang web → Micro → Cho phép (hoặc Cài đặt iOS → Safari → Micro), rồi tải lại trang.",
		desktop:
			"Bấm biểu tượng khoá/quyền bên trái thanh địa chỉ → cho phép Micro, rồi tải lại trang và thử lại.",
	},
	startError: {
		screenDenied:
			"Chưa được cấp quyền chia sẻ màn hình / micro — hãy chọn nguồn chia sẻ và cho phép truy cập, rồi thử lại.",
		micDenied: "Trình duyệt chưa được cấp quyền Micro. {hint}",
		notFound: "Không tìm thấy thiết bị micro phù hợp.",
		busy: "Micro đang được ứng dụng khác sử dụng — hãy đóng ứng dụng đó rồi thử lại.",
		aborted:
			"Không truy cập được micro (thiết bị bận hoặc bị gián đoạn) — thử lại.",
		insecure: "Ghi âm cần kết nối bảo mật (HTTPS).",
		failed: "Không bắt đầu ghi được.",
		denied: "Không bắt đầu ghi được (quyền truy cập bị từ chối?)",
	},
	interrupt: {
		micEnded:
			"Bản ghi đã dừng (mất micro — có thể do màn hình bị khóa) — đã lưu phần ghi được.",
		hidden:
			"Bản ghi đã dừng khi màn hình bị khóa / chuyển ứng dụng — đã lưu phần ghi được.",
	},
	chunksLost:
		"Một số đoạn bị lỗi mạng — đã lưu phần tải được (bản ghi có thể bị cắt ngắn).",
	noSource:
		"Không lấy được nguồn ghi nào — kiểm tra quyền mic / chia sẻ màn hình.",
	mediaError: "Lỗi ghi (MediaRecorder) — đã dừng",
	invalidSession: "Phiên ghi không hợp lệ",
	finalizeFailed: "Không hoàn tất bản ghi",
	chunkFailed: "Tải chunk thất bại",
	uploadFailed: "Lỗi tải bản ghi",
	native: {
		signInTitle: "Phiên đăng nhập đã hết",
		signInBody: "Đăng nhập lại để tải nốt các đoạn đang chờ trên máy.",
		quotaTitle: "Hết dung lượng",
		quotaBody: "{message} Các đoạn vẫn giữ trên máy và tự tải lên khi có chỗ.",
		lostTitle: "Mất phần cuối của một bản ghi",
		lostBody:
			"Ứng dụng bị đóng khi đang ghi: mất tối đa {seconds} giây của đoạn đang mở.",
		syncTitle: "Chưa đồng bộ được các đoạn ghi trên máy",
		syncRetry: "Mở lại ứng dụng để thử lại.",
	},
} as const;
