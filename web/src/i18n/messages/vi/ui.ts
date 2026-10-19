/** Shared UI primitives: toasts, loading status, theme switch, usage card, audio player. */
export const ui = {
	loading: "Đang tải…",
	theme: {
		legend: "Giao diện",
		light: "Giao diện sáng",
		dark: "Giao diện tối",
		system: "Theo hệ điều hành",
		cycle: "{current} — bấm để đổi",
	},
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
