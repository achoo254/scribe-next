/** Scribe on a domain other than hoandat.com (a tenant of the IdP). */
export const tenant = {
	disabled: {
		title: "Scribe chưa bật trên domain này",
		domain: "Domain: {domain}",
		users:
			"Chủ domain {domain} chưa bật Scribe, nên chưa đăng nhập hay ghi, chép lời ở đây được. Hãy báo cho người quản lý domain của bạn.",
		owner:
			"Bạn là chủ domain? Thêm key AI chép lời (ElevenLabs, OpenAI hoặc Google Gemini) ở <admin>trang quản trị {domain}</admin> để bật Scribe. Mọi lượt chép lời, biên bản, hỏi đáp và dịch của người dùng domain này tính vào tài khoản AI của bạn.",
	},
} as const;
