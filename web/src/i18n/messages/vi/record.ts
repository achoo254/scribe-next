/** "Ghi âm" page: the recorder (browser and iOS) and the recordings history. */
export const record = {
	title: "Ghi âm",
	description:
		"Ghi micro, tiếng hệ thống hoặc màn hình vào một bản ghi. Bản ghi được tải lên ngay trong lúc ghi và hiện ở Lịch sử ghi bên dưới.",
	nativeDescription:
		"Ghi bằng micro của iPhone, vẫn ghi khi khoá màn hình hoặc có cuộc gọi. Mỗi đoạn ghi xong được tải lên Lịch sử ghi ngay trong lúc ghi tiếp.",
	noSource: "Hãy bật ít nhất một nguồn ghi.",
	sourcesHelp:
		"Tiếng hệ thống và ghi màn hình cần chia sẻ màn hình — tốt nhất trên <b>Chrome/Edge</b>: chọn “Toàn màn hình” và bật “Chia sẻ âm thanh” để có tiếng hệ thống. Firefox/Safari có thể không hỗ trợ tiếng hệ thống. Chọn nhiều nguồn → trộn chung vào 1 bản ghi.",
	noScreenCapture:
		"Trình duyệt này không hỗ trợ chia sẻ màn hình (chỉ ghi được mic).",
	sources: {
		legend: "Nguồn",
		mic: "Micro",
		systemAudio: "Tiếng hệ thống",
		screen: "Ghi kèm màn hình (video)",
	},
	upload: {
		choose: "Tải tệp lên",
		hint: "Tệp âm thanh hoặc video có sẵn trên máy, để nghe lại và chép lời.",
		progress: "Tiến độ tải lên",
		uploading: "Đang tải “{name}” lên… {percent}%",
		resume: "Tải tiếp",
		done: "Đã tải lên “{name}” — xem và chép lời trong <b>Lịch sử ghi</b> bên dưới.",
	},
	splitAfter: {
		toggle: "Cắt thành đoạn theo phút",
		minutes: "Số phút mỗi đoạn",
		invalid: "Nhập số phút lớn hơn 0.",
	},
	live: {
		level: "Mức âm thanh",
		monitor: "Toàn màn hình",
		window: "Cửa sổ",
		browser: "Tab trình duyệt",
	},
	start: "Bắt đầu ghi",
	stop: "Dừng ghi",
	timer: "Đang ghi — {clock}",
	saving: "Đang lưu bản ghi…",
	splitting: "Đang cắt bản ghi thành các đoạn…",
	splitDone: {
		other: "Đã cắt thành {count} đoạn — xem trong <b>Lịch sử ghi</b> bên dưới.",
	},
	bookmark: {
		mark: "Đánh dấu",
		marked: { other: "Đã đánh {count} mốc" },
		waiting: { other: "{count} mốc đang chờ lưu, tự thử lại" },
		failed: { other: "{count} mốc không lưu được: {reason}" },
		dropped: { other: "{count} mốc bị bỏ vì bản ghi đã được cắt hoặc xoá" },
		unknownReason: "không rõ lý do",
	},
	native: {
		segmentLength: "Độ dài mỗi đoạn",
		segmentOption: { other: "{count} phút" },
		uploadsWaiting: {
			other:
				"Đang chờ tải lên: {count} đoạn (giữ trên máy, tự tải khi có mạng).",
		},
		uploadsFailed: {
			other: "{count} đoạn bị máy chủ từ chối — vẫn giữ trên máy.",
		},
	},
	groups: {
		expiresSoon: "sắp tự xoá",
		expiresIn: "tự xoá sau {days} ngày",
		iphoneSession: "Bản ghi trên iPhone",
	},
	history: {
		title: "Lịch sử ghi",
		downloadAll: "Tải tất cả",
		loading: "Đang tải lịch sử ghi…",
		loadFailed: "Không tải được lịch sử ghi",
		emptyTitle: "Chưa có bản ghi nào",
		emptyBody: "Bản ghi sẽ hiện ở đây sau khi bạn bấm “Dừng ghi”.",
		retention: {
			other:
				"Bản ghi được <b>tự động xoá sau {count} ngày</b> kể từ khi tạo. Hãy tải về nếu muốn lưu giữ lâu dài.",
		},
		downloadDay: { other: "Tải ngày ({count})" },
		deleted: "Đã xoá “{name}”",
		undo: "Hoàn tác",
		deleteAll: "Xoá tất cả",
		deleteAllTitle: "Xoá toàn bộ lịch sử ghi",
		deleteAllQuestion: { other: "Xoá {count} tệp, tổng {size}?" },
		deleteAllBody:
			"Mọi bản ghi và tệp tải lên cùng bản chép lời, biên bản, dấu mốc đi kèm sẽ bị xoá vĩnh viễn. Không thể hoàn tác.",
		deletedAll: { other: "Đã xoá {count} tệp" },
		deletedAllSkipped: {
			other: "Giữ lại {count} tệp của bản ghi đang ghi hoặc đang cắt.",
		},
	},
	row: {
		screen: "Màn hình",
		mic: "Mic",
		upload: "Tệp tải lên",
		delete: "Xoá {name}",
		play: "Nghe lại",
		hide: "Ẩn",
		menu: "Thao tác với {name}",
		download: "Tải về",
		saveToFiles: "Lưu vào Tệp",
	},
	session: {
		badge: "iPhone",
		meta: { other: "{count} đoạn · {duration} · {size}" },
		expand: "Xem các đoạn",
		collapse: "Thu gọn",
		part: "Phần {number}",
		delete: "Xoá cả phiên {name}",
	},
} as const;
