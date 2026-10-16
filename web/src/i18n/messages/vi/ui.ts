/** Shared UI primitives: toasts, loading status, theme switch, storage bar, audio player. */
export const ui = {
	loading: "Đang tải…",
	theme: {
		legend: "Giao diện",
		light: "Giao diện sáng",
		dark: "Giao diện tối",
		system: "Theo hệ điều hành",
	},
	storage: {
		label: "Dung lượng",
		meter: "Dung lượng đã dùng",
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
