/** Fallback texts of failed requests, used when the backend sends no `{ error }` of its own. */
export const errors = {
	network: "Không kết nối được máy chủ — kiểm tra mạng rồi thử lại.",
	requestFailed: "Yêu cầu thất bại",
	generic: "Đã có lỗi xảy ra.",
	files: {
		history: "Không tải được lịch sử ghi",
		list: "Không tải được danh sách tệp",
		delete: "Không xoá được tệp",
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
		saveSegment: "Không lưu được câu đã sửa",
		delete: "Không xoá được bản chép lời",
	},
	split: {
		create: "Không thể tạo công việc cắt file",
		progress: "Không thể lấy tiến độ công việc",
		failed: "Cắt file thất bại",
		watch: "Lỗi khi theo dõi tiến độ",
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
