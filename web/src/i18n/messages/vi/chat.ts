/** Linking the account to chat.hoandat.com, where the minutes are sent. */
export const chat = {
	menu: {
		link: "Liên kết chat.hoandat.com",
		unlink: "Gỡ liên kết chat",
		unlinked: "Đã gỡ liên kết chat",
		unlinkFailed: "Không gỡ được liên kết chat",
		unlinkFrom: "Gỡ liên kết chat ({name})",
	},
	dialog: {
		title: "Liên kết chat.hoandat.com",
		intro:
			"Liên kết một lần để Scribe gửi biên bản vào chat riêng của bạn với bot Scribe trên chat.hoandat.com.",
		step1: "Bấm “Mở chat” (trình duyệt sẽ mở chat.hoandat.com).",
		step2:
			"Đăng nhập nếu được hỏi, rồi bấm “Gửi /start” trong chat với bot Scribe.",
		step3: "Bot hỏi lại kèm email của bạn: bấm “Xác nhận liên kết”.",
		step4: "Quay lại đây: Scribe tự nhận ra khi đã liên kết xong.",
		steps: "Các bước liên kết",
		expires: "Đường dẫn chỉ dùng một lần, hết hạn lúc {time}.",
		preparing: "Đang tạo đường dẫn liên kết…",
		open: "Mở chat",
		opensBrowser: ", mở ở trình duyệt",
		waiting: "Đang chờ bạn gửi /start rồi bấm “Xác nhận liên kết” trong chat…",
		linked:
			"Đã liên kết. Biên bản sẽ được gửi vào chat riêng của bạn với bot Scribe.",
		linkedTitle: "Đã liên kết chat",
	},
} as const;
