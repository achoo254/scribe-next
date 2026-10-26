/**
 * Public landing at `/` for visitors who are not signed in. Every feature
 * named here exists in the app today; the plan limits repeat the public
 * terms (`legal.terms.plans`), so change both together.
 */
export const landing = {
	nav: {
		label: "Giới thiệu",
		features: "Tính năng",
		how: "Cách dùng",
		signIn: "Đăng nhập",
	},
	hero: {
		eyebrow: "Ghi âm và chép lời tiếng Việt",
		title: "Nói. Nhận bản chép lời.",
		lede: "Scribe ghi âm ngay trên trình duyệt hoặc iPhone, cắt tệp audio, video dài thành từng đoạn, rồi chép lời tiếng Việt theo từng người nói để bạn tìm, sửa và tải về.",
		start: "Bắt đầu miễn phí",
		features: "Xem tính năng",
		note: "Đăng nhập bằng tài khoản {idpName} (Google hoặc GitHub).",
	},
	features: {
		eyebrow: "Tính năng",
		title: "Một chỗ cho cả cuộc họp",
		lead: "Từ lúc bấm ghi tới khi có biên bản, mọi bước nằm trong một ứng dụng.",
		record: {
			title: "Ghi âm trên web và iPhone",
			body: "Ghi micro, hoặc âm thanh và màn hình máy tính ngay trên trình duyệt. Trên iPhone, Scribe vẫn ghi khi bạn khoá màn hình.",
		},
		split: {
			title: "Cắt file audio, video",
			body: "Tải lên một tệp dài, cắt theo số phút, dung lượng hoặc số phần bằng nhau, rồi tải các đoạn về dạng zip.",
		},
		transcribe: {
			title: "Chép lời tiếng Việt",
			body: "Lời chép chia theo người nói, có mốc thời gian để nghe lại. Sửa câu, đặt tên người nói, xuất TXT, SRT, VTT, Markdown hoặc Word.",
		},
		after: {
			title: "Biên bản, tìm kiếm, hỏi đáp",
			body: "Tóm tắt thành biên bản, tìm trong mọi lời đã chép, hỏi Scribe và nhận câu trả lời kèm mốc giây, dịch lời chép Việt ↔ Anh.",
		},
	},
	how: {
		eyebrow: "Cách dùng",
		title: "Ba bước tới bản chép lời",
		steps: {
			signIn: {
				title: "Đăng nhập",
				body: "Dùng tài khoản {idpName}, không cần tạo mật khẩu mới.",
			},
			capture: {
				title: "Ghi hoặc tải tệp lên",
				body: "Ghi ngay trên web, trên iPhone, hoặc chọn một tệp audio, video có sẵn.",
			},
			read: {
				title: "Nhận bản chép lời",
				body: "Bấm Chép lời, rồi đọc, sửa, tìm và tải về khi xong.",
			},
		},
	},
	cta: {
		title: "Thử với cuộc họp tiếp theo",
		body: "Gói miễn phí có 200 MB lưu trữ; tệp tự xoá sau 7 ngày, nên hãy tải về bản bạn cần giữ.",
	},
	footer: {
		madeBy: "Một app của <home>hoandat.com</home>",
	},
} as const;
