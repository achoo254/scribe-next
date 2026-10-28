/** Fallback texts of failed requests, used when the backend sends no `{ error }` of its own. */
export const errors = {
	network: "Không kết nối được máy chủ — kiểm tra mạng rồi thử lại.",
	requestFailed: "Yêu cầu thất bại",
	generic: "Đã có lỗi xảy ra.",
	files: {
		history: "Không tải được lịch sử ghi",
		list: "Không tải được danh sách tệp",
		delete: "Không xoá được tệp",
		deleteAll: "Không xoá được lịch sử ghi",
		usage: "Không tải được dung lượng",
		download: "Không tải được tệp",
	},
	transcripts: {
		request: "Không gửi được yêu cầu chép lời",
		load: "Không tải được bản chép lời",
		list: "Không tải được danh sách bản chép lời",
		doc: "Không tải được bản chép lời",
		progress: "Không tải được tiến độ chép lời",
		rename: "Không đổi được tên",
		speakers: "Không đổi được tên người nói",
		saveSegment: "Không lưu được câu đã sửa",
		delete: "Không xoá được bản chép lời",
		summary: "Không tóm tắt được",
		share: "Không gửi được sang chat",
		bookmark: "Không lưu được dấu mốc",
		deleteBookmark: "Không xoá được dấu mốc",
		translate: "Không dịch được",
		translation: "Không tải được bản dịch",
		translationProgress: "Không tải được tiến độ dịch",
	},
	search: {
		search: "Không tìm được trong bản chép lời",
		ask: "Không hỏi được Scribe",
	},
	keys: {
		load: "Không tải được key riêng",
		save: "Không lưu được key",
		delete: "Không xoá được key",
		prefs: "Không đổi được lựa chọn",
	},
	chat: {
		status: "Không kiểm được liên kết chat",
		link: "Không tạo được đường dẫn liên kết",
		unlink: "Không gỡ được liên kết chat",
	},
	split: {
		create: "Không thể tạo công việc cắt file",
		progress: "Không thể lấy tiến độ công việc",
		failed: "Cắt file thất bại",
		watch: "Lỗi khi theo dõi tiến độ",
	},
	/**
	 * What a refusal of the backend says in a language other than
	 * Vietnamese: the backend writes its `{ error }` in Vietnamese, so
	 * `lib/server-errors.ts` words the coded ones (and a few statuses) here.
	 * These Vietnamese texts repeat the backend's own wording.
	 */
	server: {
		quotaExceeded:
			"Không đủ phút chép lời: tháng này đã dùng {used}/{limit} phút.",
		dailyLimit:
			"Gói miễn phí chép được {limit} phút mỗi ngày, hôm nay đã dùng {used} phút. Mở lại lúc {time}.",
		tooLongForFree:
			"Gói miễn phí chép tối đa {limit} phút mỗi ngày, các bản ghi đã chọn dài {used} phút. Chọn ít bản ghi hơn.",
		accountTooNew: "Tài khoản mới dùng được chép lời miễn phí từ {time}.",
		providerHours: "Đã dùng hết giờ chép lời của tháng này, mở lại lúc {time}.",
		notTranscribed: "Bản ghi chưa có đoạn nào chép lời xong. Chép lời trước.",
		confirmRequired: "Gõ từ xác nhận để xoá tài khoản.",
		chatLinkDisabled: "Liên kết chat chưa được bật trên máy chủ này.",
		chatNotLinked: "Chưa liên kết chat.hoandat.com.",
		chatBlocked:
			"Không gửi được vì bot Scribe đã bị chặn trên chat. Đã gỡ liên kết, hãy liên kết lại.",
		noSummary: "Chưa có biên bản. Bấm Tóm tắt trước rồi gửi.",
		bookmarksFull: "Bản ghi này đã có số dấu mốc tối đa.",
		realtimeNotAllowed: "Tài khoản này chưa được dùng chép trực tiếp.",
		userKeyFailed:
			"Không dùng được key riêng của bạn. Kiểm tra key trong Key riêng.",
		keyFormat: "Key không đúng định dạng của nhà cung cấp.",
		keyRejected: "Nhà cung cấp đã từ chối key này.",
		keyNoCredit: "Tài khoản của key này đã hết credit.",
		keyCheckFailed: "Không kiểm được key lúc này, thử lại sau.",
		accountDeleted: "Tài khoản Scribe đã bị xoá.",
		signedOut: "Phiên đăng nhập đã hết. Đăng nhập lại.",
		tooLarge:
			"Tệp quá lớn hoặc đã hết dung lượng lưu trữ. Xoá bớt bản ghi hoặc tệp để ghi và tải lên tiếp.",
		tooMany: "Thao tác quá nhanh, đợi một chút rồi thử lại.",
		unavailable: "Tính năng này đang tạm ngưng, thử lại sau.",
	},
	upload: {
		status: "Không thể lấy trạng thái upload",
		chunk: "Tải lên thất bại, có thể thử lại",
		noProgress: "Máy chủ không ghi thêm dữ liệu — thử tải lên lại.",
		complete: "Không thể hoàn tất upload",
		init: "Không thể khởi tạo upload (file quá lớn hoặc vượt hạn mức)",
		failed: "Tải lên thất bại",
	},
} as const;
