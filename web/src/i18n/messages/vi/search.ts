/** "Kho bản ghi": search every transcript, and "Hỏi Scribe" with timed sources. */
export const search = {
	entry: {
		label: "Tìm trong bản chép lời",
		placeholder: "Tìm câu, ví dụ: hop dong",
		submit: "Tìm",
	},
	page: {
		back: "Lời đã chép",
		title: "Tìm trong bản ghi",
		description:
			"Tìm câu trong mọi bản chép lời của bạn; gõ có dấu hay không dấu đều được. Bấm một câu để nghe từ đó.",
		field: "Tìm trong bản chép lời",
		placeholder: "Ví dụ: hợp đồng thuê nhà",
	},
	results: {
		title: "Kết quả",
		count: { other: "{count} câu khớp" },
		showing: "{count} câu khớp nhất trong {total}",
		searching: "Đang tìm…",
		idle: "Gõ vài từ để tìm trong các bản chép lời.",
		tooShort: "Gõ ít nhất 2 ký tự để tìm.",
		emptyTitle: "Không có câu nào khớp",
		emptyBody: "Thử từ khác hoặc ít từ hơn. Chỉ tìm trong câu đã chép lời.",
		failed: "Không tìm được",
	},
	ask: {
		title: "Hỏi Scribe",
		introAll:
			"Hỏi bằng câu tự nhiên trên mọi bản ghi của bạn. Câu trả lời chỉ dựa vào bản ghi và trích mốc giây để nghe lại.",
		introOne:
			"Hỏi về bản ghi này bằng câu tự nhiên. Câu trả lời chỉ dựa vào bản ghi và trích mốc giây để nghe lại.",
		field: "Câu hỏi cho Scribe",
		placeholderAll: "Ví dụ: Khách chốt giá bao nhiêu?",
		placeholderOne: "Ví dụ: Ai nhận gửi báo giá?",
		submit: "Hỏi",
		running: "Đang tìm câu trả lời… có thể mất vài chục giây.",
		failed: "Không trả lời được",
		question: "Bạn hỏi: {question}",
		sources: "Nguồn",
		cite: "Nguồn {n}: {title}, nghe từ {time}",
		session: "Các câu đã hỏi chỉ giữ tới khi rời trang.",
		dailyLimit: "Hôm nay đã dùng {used}/{limit} lượt hỏi. Mở lại lúc {time}.",
		monthlyLimit:
			"Tháng này tài khoản đã dùng hết hạn mức tóm tắt và hỏi đáp ({used}/{limit} phút quy đổi). Mở lại lúc {time}.",
		capReached:
			"Hỏi đáp đã dùng hết hạn mức chung của tháng này, mở lại lúc {time}.",
	},
} as const;
