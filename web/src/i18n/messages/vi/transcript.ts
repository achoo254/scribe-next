/** "Transcript" pages: the list with the minutes left, and the reader's frame. */
export const transcript = {
	list: {
		title: "Transcript",
		description:
			"Bản chép lời của các bản ghi, mới nhất ở trên. Mở một transcript để đọc, nghe lại từ một câu và sửa chữ.",
		loading: "Đang tải transcript…",
		loadFailed: "Không tải được danh sách transcript",
		emptyTitle: "Chưa có transcript nào",
		emptyBody:
			"Bấm “Chép lời” trên một bản ghi ở trang Ghi âm; transcript sẽ hiện ở đây.",
		emptyAction: "Mở trang Ghi âm",
		segments: { other: "{count} đoạn" },
	},
	quota: {
		left: "Còn <b>{left}</b>/{limit} phút chép lời tháng này",
		resets: "đặt lại ngày {date}",
		meter: "Phút chép lời đã dùng",
		paused: "Chép lời đang tạm ngưng, quản trị đã được báo. Thử lại sau.",
		capReached:
			"Chép lời đã dùng hết hạn mức chung của tháng này, sẽ mở lại đầu tháng sau.",
	},
	reader: {
		back: "Transcript",
		loading: "Đang tải transcript…",
		errorTitle: "Không mở được transcript",
		loadFailed: "Không tải được nội dung",
		rename: "Đổi tên",
		titleField: "Tên transcript",
		titleHint: "Để trống để dùng tên mặc định.",
		saveTitle: "Lưu tên",
		renamed: "Đã đổi tên transcript",
		menu: "Thao tác với transcript",
		downloadTxt: "Tải .txt",
		downloadSrt: "Tải .srt",
		saveTxt: "Lưu .txt",
		saveSrt: "Lưu .srt",
		delete: "Xoá transcript",
		deleteTitle: "Xoá transcript",
		deleteQuestion: "Xoá transcript “{name}”?",
		deleteBody:
			"Phần chữ và phần đã sửa sẽ mất. Audio vẫn còn trong Lịch sử ghi.",
		deleted: "Đã xoá transcript “{name}”",
	},
} as const;
