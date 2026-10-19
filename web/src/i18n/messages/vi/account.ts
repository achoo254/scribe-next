/** "Xoá tài khoản" in the account menu (App Store guideline 5.1.1(v)). */
export const account = {
	menu: "Xoá tài khoản",
	delete: {
		title: "Xoá tài khoản Scribe",
		question: "Xoá vĩnh viễn tài khoản Scribe của {email}?",
		intro: "Scribe xoá ngay trên máy chủ:",
		files: "Mọi bản ghi, tệp tải lên và đoạn cắt.",
		transcripts:
			"Lời chép, câu đã sửa, tên người nói, biên bản, dấu mốc, bản dịch và chỉ mục tìm kiếm.",
		keys: "Key riêng và liên kết chat.hoandat.com.",
		sessions:
			"Phiên đăng nhập trên mọi thiết bị và số liệu mức dùng của tài khoản.",
		undo: "Không hoàn tác được. Hãy tải về những gì cần giữ trước khi xoá.",
		audit:
			"Nhật ký bảo mật (thời điểm, hành động, địa chỉ IP) được giữ tới hết 180 ngày nhưng không còn email hay tên tệp.",
		idp: "Tài khoản hoandat dùng chung cho các ứng dụng hoandat và không bị xoá ở đây. Muốn xoá cả tài khoản đó, vào <account>auth.hoandat.com/account</account>.",
		busy: "Đang ghi hoặc còn tệp đang tải lên. Dừng ghi và chờ tải xong rồi mới xoá tài khoản.",
		confirmWord: "XOÁ",
		confirmLabel: "Gõ {word} để xác nhận",
		confirm: "Xoá tài khoản",
		recordingOpen: "Đang có bản ghi chạy. Dừng ghi rồi thử lại.",
	},
} as const;
