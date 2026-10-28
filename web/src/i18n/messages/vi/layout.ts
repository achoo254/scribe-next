/** App shell: top bar, account menu, sign-out and leave confirmations, footer. */
export const layout = {
	skipLink: "Bỏ qua tới nội dung chính",
	breadcrumb: "Đường dẫn",
	appHeader: {
		home: "Scribe, về trang Ghi âm",
		nav: "Chức năng",
		record: "Ghi âm",
		split: "Cắt file",
		transcript: "Lời đã chép",
	},
	accountMenu: {
		trigger: "{email} · Tài khoản",
		signOut: "Đăng xuất",
	},
	idpMenu: {
		account: "Tài khoản {idpName}",
		apps: "Ứng dụng",
		opensIdp: ", mở trang {idpName} ở thẻ mới",
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
		ecosystem: "Một app của hoandat.com",
	},
	leaveGuard: {
		title: "Rời trang",
		question: "Rời trang khi chưa lưu?",
		detail: "Phần đang sửa chưa được lưu sẽ mất.",
		confirm: "Vẫn rời trang",
	},
	pageError: {
		title: "Trang này gặp lỗi",
		body: "Bản ghi đang chạy (nếu có) vẫn tiếp tục. Mở trang khác hoặc tải lại trang.",
		reload: "Tải lại trang",
	},
} as const;
