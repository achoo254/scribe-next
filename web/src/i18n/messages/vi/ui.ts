/** Shared UI primitives: toasts, loading status, usage card, audio player. */
export const ui = {
	loading: "Đang tải…",
	usage: {
		region: "Mức dùng",
		retention: { other: "Tệp tự xoá sau {count} ngày" },
		storage: "Lưu trữ",
		storageMeter: "Dung lượng đã dùng",
		ownKey: {
			transcribe: "Chép lời",
			text: "Biên bản, hỏi đáp, dịch",
			value: "Dùng key của bạn ({provider})",
			minutes: { other: "{count} phút bằng key của bạn tháng này" },
		},
		storageFull:
			"Đã đầy dung lượng: xoá bớt bản ghi hoặc tệp để ghi và tải lên tiếp.",
	},
	player: {
		playFailed: "Không phát được: {error}",
		loadingRecording: "Đang tải bản ghi để phát…",
		loadingAudio: "Đang tải audio…",
		play: "Phát",
		pause: "Tạm dừng",
		seek: "Tua",
		mute: "Tắt tiếng",
		unmute: "Bật tiếng",
		volume: "Âm lượng",
	},
	toast: {
		region: "Thông báo",
		close: "Đóng thông báo",
	},
} as const;
