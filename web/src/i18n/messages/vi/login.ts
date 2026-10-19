/** Sign-in page (`/login`). */
export const login = {
	title: "Đăng nhập",
	subtitle: "Ghi âm, cắt file và chép lời bằng tài khoản hoandat.",
	submit: "Đăng nhập bằng hoandat",
	note: "Dùng tài khoản hoandat (Google hoặc GitHub).",
	sessionCheck: "Đang kiểm tra phiên đăng nhập…",
	noSession: "Đăng nhập xong nhưng máy chủ chưa nhận phiên — thử lại.",
	/** Codes the backend appends to /login?error=… */
	errors: {
		denied: "Bạn đã từ chối đăng nhập ở trang tài khoản.",
		expired: "Lượt đăng nhập đã hết hạn — bấm Đăng nhập lại.",
		invalid: "Phản hồi đăng nhập không hợp lệ — bấm Đăng nhập lại.",
		unverified:
			"Email của tài khoản chưa được xác minh — đăng nhập bằng Google hoặc GitHub đã xác minh email.",
		forbidden: "Tài khoản này chưa được phép dùng Scribe.",
		idp: "Máy chủ đăng nhập báo lỗi — thử lại sau.",
		busy: "Quá nhiều lượt đăng nhập — đợi một phút rồi thử lại.",
		failed: "Đăng nhập thất bại — thử lại sau.",
	},
	failed: "Đăng nhập thất bại",
	accountDeleted:
		"Đã xoá tài khoản Scribe và mọi dữ liệu của bạn. Tài khoản hoandat vẫn còn ở auth.hoandat.com.",
	accountDeletedPending:
		"Đã xoá tài khoản Scribe. Phần dữ liệu còn lại sẽ được xoá xong trong vòng một giờ.",
} as const;
