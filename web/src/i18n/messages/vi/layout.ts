/** App shell: top bar, account menu, sign-out confirmation, footer. */
export const layout = {
	skipLink: "Bỏ qua tới nội dung chính",
	appHeader: {
		home: "Scribe, về trang Ghi âm",
		nav: "Chức năng",
		record: "Ghi âm",
		split: "Cắt file",
		transcript: "Transcript",
	},
	accountMenu: {
		trigger: "{email} · Tài khoản",
		signOut: "Đăng xuất",
	},
	idpMenu: {
		account: "Tài khoản hoandat",
		apps: "Ứng dụng",
		opensIdp: ", mở trang hoandat ở thẻ mới",
	},
	signOutConfirm: {
		title: "Đăng xuất",
		question: "Đăng xuất khi chưa xong việc?",
		recording: "Bản ghi đang chạy sẽ dừng; phần đã ghi vẫn được lưu.",
		segmentsWaiting: {
			other:
				"{count} đoạn chưa tải lên vẫn giữ trên máy và tải tiếp khi bạn đăng nhập lại.",
		},
		fileUploading: "Tệp đang tải lên ở Cắt file sẽ dừng giữa chừng.",
		confirm: "Vẫn đăng xuất",
	},
	footer: {
		links: "Liên kết tài khoản",
	},
} as const;
