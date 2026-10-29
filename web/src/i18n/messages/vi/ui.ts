/** Shared UI primitives: toasts, loading status, usage card, audio player. */
export const ui = {
	loading: "Đang tải…",
	usage: {
		region: "Gói và mức dùng",
		plan: { free: "Gói Free", plus: "Gói Plus" },
		planUntil: "đến {date}",
		retention: { other: "Tệp tự xoá sau {count} ngày" },
		resets: "phút đặt lại ngày {date}",
		storage: "Lưu trữ",
		storageMeter: "Dung lượng đã dùng",
		month: "Chép lời tháng này",
		monthMeter: "Phút chép lời đã dùng tháng này",
		today: "Hôm nay",
		todayMeter: "Phút chép lời đã dùng hôm nay",
		minutes: "{used}/{limit} phút",
		ownKey: {
			transcribe: "Chép lời",
			text: "Biên bản, hỏi đáp, dịch",
			value: "Dùng key của bạn ({provider})",
			minutes: { other: "{count} phút bằng key của bạn tháng này" },
		},
		sharedLow: {
			other:
				"Giờ chép lời chung của hệ thống chỉ còn khoảng {count} phút tới ngày gia hạn, có thể hết trước phút của gói bạn.",
		},
		sharedOut:
			"Hệ thống đã dùng hết giờ chép lời chung, mở lại ngày {date}. Phút của gói bạn vẫn được giữ.",
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
