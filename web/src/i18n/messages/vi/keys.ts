/** "Key riêng": the user's own API keys (ElevenLabs, OpenAI, Gemini, Claude). */
export const keys = {
	menu: "Key riêng",
	dialog: {
		title: "Key riêng",
		intro:
			"Dùng API key của chính bạn thay cho key của Scribe. Key được mã hoá và lưu trên máy chủ Scribe, không bao giờ hiện lại; nhà cung cấp tính tiền vào tài khoản của bạn và việc chạy bằng key của bạn không trừ phút hay lượt của Scribe.",
		unavailable:
			"Máy chủ chưa bật Key riêng. Quản trị cần tạo tệp bí mật mã hoá rồi khởi động lại.",
		loading: "Đang tải key riêng…",
		saved: "••••{last4} · đã kiểm {date}",
		none: "Chưa có key",
		status: {
			invalid: "Nhà cung cấp đã từ chối key này: nhập key mới hoặc xoá.",
			quota: "Tài khoản của key đã hết credit: nạp thêm hoặc nhập key khác.",
			unreadable: "Máy chủ không đọc được key đã lưu: hãy nhập lại.",
		},
		field: "Key {provider}",
		placeholderNew: "Dán key",
		placeholderReplace: "Dán key mới để thay",
		save: "Lưu và kiểm",
		delete: "Xoá key",
		savedToast: "Đã kiểm và lưu key {provider}",
		deletedToast: "Đã xoá key {provider}",
		create: "Tạo key ở {provider}",
		opensBrowser: ", mở ở thẻ mới",
	},
	provider: {
		elevenlabs: {
			name: "ElevenLabs",
			usedFor:
				"Chép lời tệp và cả phiên iPhone (có phân biệt người nói), chép trực tiếp. Key cần quyền Speech to Text.",
		},
		openai: {
			name: "OpenAI",
			usedFor:
				"Biên bản, hỏi đáp, dịch; chép lời tệp (gpt-transcribe, không phân biệt người nói); chép trực tiếp.",
		},
		gemini: {
			name: "Google Gemini",
			usedFor:
				"Biên bản, hỏi đáp, dịch; chép lời tệp (không phân biệt người nói, mốc giờ của câu là ước lượng). Không dùng cho chép trực tiếp.",
		},
		anthropic: {
			name: "Anthropic Claude",
			usedFor: "Biên bản, hỏi đáp, dịch. Claude không chép lời.",
		},
	},
	uses: {
		title: "Dùng key nào cho việc gì",
		hint: "Chỉ chọn được nhà cung cấp đã có key hợp lệ.",
		transcribe: "Chép lời tệp",
		live: "Chép trực tiếp trên iPhone",
		text: "Biên bản, hỏi đáp, dịch",
		scribe: "Đang dùng key của Scribe, tính vào mức dùng của gói.",
		needsKey: "{provider} (chưa có key)",
		model: "Model",
		geminiNote: "Gemini: mốc giờ của câu là ước lượng, không có người nói.",
	},
} as const;
