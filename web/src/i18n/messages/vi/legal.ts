/**
 * Public legal pages (`/privacy`, `/terms`). A section body is paragraphs
 * separated by a blank line; a paragraph whose lines all start with "- " is
 * a list. Tags open links (see `pages/legal-page.tsx`): one tag per
 * destination, so a URL lives in code, not in the text.
 */
export const legal = {
	effective: "Có hiệu lực từ {date}",
	nav: {
		label: "Văn bản pháp lý",
		privacy: "Quyền riêng tư",
		terms: "Điều khoản",
	},
	contactFallback: "người vận hành Scribe qua tài khoản {idpName}",
	englishTitle: "English summary",
	privacy: {
		title: "Chính sách quyền riêng tư",
		intro:
			"Scribe (trang web scribe.hoandat.com và ứng dụng Scribe trên iPhone) ghi âm, cắt tệp và chép lời cuộc họp bằng tiếng Việt. Văn bản này nói Scribe thu thập dữ liệu gì, dùng vào việc gì, gửi cho ai, giữ bao lâu và bạn có những quyền gì. “Chúng tôi” là người vận hành Scribe.",
		collect: {
			title: "Dữ liệu chúng tôi thu thập",
			body: "- <b>Tài khoản:</b> email, tên hiển thị và mã định danh tài khoản {idpName} mà auth.hoandat.com gửi cho Scribe khi bạn đăng nhập; gói đang dùng.\n- <b>Nội dung bạn tạo:</b> bản ghi âm, bản ghi màn hình, tệp bạn tải lên, lời chép, câu bạn sửa, tên người nói, tiêu đề, biên bản, dấu mốc và bản dịch.\n- <b>Câu hỏi “Hỏi Scribe”:</b> chỉ dùng để trả lời rồi bỏ, không lưu.\n- <b>Key riêng:</b> key API bạn tự nhập, lưu mã hoá (AES-256-GCM); Scribe chỉ hiện lại 4 ký tự cuối, không bao giờ gửi key về trình duyệt hay ứng dụng.\n- <b>Mức dùng:</b> dung lượng, số phút chép lời, số lượt tóm tắt, hỏi đáp và dịch.\n- <b>Nhật ký bảo mật:</b> thời điểm, hành động (đăng nhập, tải lên, xoá…) và địa chỉ IP; nhật ký truy cập kỹ thuật của máy chủ.\n- <b>Trên iPhone:</b> đoạn ghi âm nằm trên máy cho tới khi tải lên xong; mã đăng nhập của thiết bị nằm trong Keychain. Ứng dụng chỉ dùng micro khi bạn ghi, camera và thư viện ảnh khi bạn tự chọn quay hoặc chọn tệp để tải lên, và chỉ ghi vào thư viện Ảnh khi bạn chọn “Lưu video” trong bảng chia sẻ; không đọc danh bạ hay vị trí.\n\nScribe không có quảng cáo, không dùng bộ công cụ theo dõi hay phân tích trong ứng dụng, không bán dữ liệu và không theo dõi bạn trên ứng dụng hay trang web khác. Trang web scribe.hoandat.com dùng Cloudflare Web Analytics để đếm lượt xem trang; công cụ này không đặt cookie và không lưu gì trên thiết bị của bạn.",
		},
		purposes: {
			title: "Chúng tôi dùng dữ liệu để làm gì",
			body: "- Cung cấp các tính năng bạn dùng: ghi, lưu, phát, cắt, chép lời, chép trực tiếp, biên bản, tìm kiếm, hỏi đáp, dịch, xuất tệp và gửi biên bản sang chat.\n- Tính hạn mức của gói và chống lạm dụng.\n- Giữ an toàn cho tài khoản và xử lý sự cố.\n\nChúng tôi không dùng nội dung của bạn để huấn luyện mô hình và không đọc nội dung của bạn, trừ khi bạn yêu cầu hỗ trợ cho một bản ghi cụ thể.",
		},
		processors: {
			title: "Bên xử lý dữ liệu",
			body: "Dữ liệu chỉ được gửi tới các bên dưới đây, và chỉ để làm đúng việc bạn yêu cầu:\n\n- <b>Máy chủ của Scribe</b> (máy chủ thuê tại Việt Nam): lưu tệp, lời chép và cơ sở dữ liệu.\n- <b>Cloudflare</b>: chuyển tiếp lưu lượng và mã hoá HTTPS giữa thiết bị của bạn và máy chủ. <cloudflare>Chính sách của Cloudflare</cloudflare>.\n- <b>ElevenLabs</b>: nhận âm thanh khi bạn bấm Chép lời, và âm thanh trực tiếp khi bạn bật Chép trực tiếp trên iPhone. <elevenlabs>Chính sách của ElevenLabs</elevenlabs>.\n- <b>OpenAI</b>: hiện là nhà cung cấp mô hình chữ của máy chủ — nhận nội dung lời chép khi bạn bấm Tóm tắt, Hỏi Scribe hoặc Dịch; nhận âm thanh khi máy chủ dùng OpenAI làm đường chép lời dự phòng. <openai>Chính sách của OpenAI</openai>. Máy chủ có thể được cấu hình dùng Google Gemini hoặc Anthropic Claude thay cho OpenAI ở việc này; khi đổi, chính sách này được cập nhật trước.\n- <b>Nhà cung cấp của Key riêng:</b> khi bạn thêm key của mình (OpenAI, Google Gemini, Anthropic Claude hoặc ElevenLabs), việc tương ứng chạy bằng tài khoản của bạn ở nhà cung cấp đó, theo điều khoản bạn đã chấp nhận với họ. <google>Chính sách của Google</google>, <anthropic>chính sách của Anthropic</anthropic>.\n- <b>Tài khoản {idpName}</b> (auth.hoandat.com): đăng nhập.\n- <b>chat.hoandat.com</b>: chỉ khi bạn liên kết chat; nhận biên bản bạn chọn gửi. Khi một tài khoản chạm giới hạn miễn phí nhiều lần, email của tài khoản được gửi tới kênh báo động riêng của người vận hành trên chat.hoandat.com.",
		},
		retention: {
			title: "Giữ dữ liệu bao lâu",
			body: "- <b>Tệp và mọi thứ đi kèm</b> (lời chép, câu sửa, biên bản, dấu mốc, bản dịch, chỉ mục tìm kiếm) tự xoá sau 7 ngày với gói miễn phí, 14 ngày với gói Plus, tính từ lúc tạo. Scribe là chỗ làm việc tạm, không phải kho lưu trữ: hãy tải về bản bạn cần giữ.\n- <b>Tài khoản, Key riêng, liên kết chat:</b> tới khi bạn xoá.\n- <b>Nhật ký bảo mật:</b> 180 ngày.\n- <b>Số liệu mức dùng theo tháng:</b> tới khi bạn xoá tài khoản. Sau khi xoá, chỉ còn số liệu chi phí đã ẩn danh (không còn gắn với bạn hay bản ghi nào) để đối soát chi phí của dịch vụ.\n- <b>Bản sao lưu:</b> trước một số lần cập nhật máy chủ, người vận hành chép cơ sở dữ liệu (không gồm tệp âm thanh) vào thư mục sao lưu trên chính máy chủ để khôi phục khi có lỗi; các bản này được xoá khi bản cập nhật đã ổn định. Dữ liệu bạn đã xoá có thể còn trong một bản sao lưu như vậy tới khi bản đó bị xoá.\n- Các bên xử lý giữ dữ liệu theo chính sách của họ.",
		},
		rights: {
			title: "Quyền của bạn",
			body: "- <b>Xem và tải về:</b> phát và tải từng bản ghi, “Tải tất cả” ở Lịch sử ghi, xuất lời chép dạng TXT, SRT, VTT, Markdown hoặc Word.\n- <b>Sửa:</b> sửa câu, tiêu đề, tên người nói.\n- <b>Xoá:</b> xoá từng bản ghi, “Xoá tất cả”, xoá Key riêng, gỡ liên kết chat.\n- <b>Xoá tài khoản:</b> trong menu tài khoản chọn “Xoá tài khoản”. Scribe xoá ngay mọi tệp, lời chép, biên bản, bản dịch, dấu mốc, chỉ mục tìm kiếm, Key riêng, liên kết chat, phiên đăng nhập và số liệu mức dùng của bạn. Nhật ký bảo mật được giữ tới hết 180 ngày nhưng không còn email, tên tệp hay nội dung lỗi; số liệu chi phí được giữ ở dạng ẩn danh. Tài khoản {idpName} dùng chung cho nhiều ứng dụng; xoá tài khoản đó ở <account>auth.hoandat.com/account</account>.\n- Yêu cầu khác về dữ liệu (ví dụ bản sao dữ liệu tài khoản): liên hệ theo mục Liên hệ.",
		},
		security: {
			title: "Bảo mật",
			body: "Kết nối dùng HTTPS; phiên đăng nhập dùng cookie HttpOnly và thu hồi được; Key riêng được mã hoá; mỗi tài khoản chỉ thấy dữ liệu của chính mình. Không hệ thống nào an toàn tuyệt đối; nếu phát hiện sự cố ảnh hưởng tới dữ liệu của bạn, chúng tôi sẽ báo cho bạn.",
		},
		children: {
			title: "Trẻ em",
			body: "Scribe không dành cho trẻ em dưới 13 tuổi và không cố ý thu thập dữ liệu của trẻ dưới 13 tuổi. Người từ 13 tới dưới 16 tuổi chỉ dùng khi có sự đồng ý của cha mẹ hoặc người giám hộ. Nếu bạn biết một trẻ em đã dùng Scribe, hãy liên hệ để chúng tôi xoá dữ liệu.",
		},
		changes: {
			title: "Thay đổi chính sách",
			body: "Khi chính sách thay đổi, ngày hiệu lực ở đầu trang đổi theo; thay đổi quan trọng được báo trong ứng dụng trước khi áp dụng.",
		},
		english:
			"Scribe records meetings and transcribes Vietnamese. We collect your {idpName} account email and name, the recordings and files you create, their transcripts and the things you make from them (edits, minutes, bookmarks, translations), your own API keys (encrypted, never shown back), usage counts, and security logs (time, action, IP address). Audio goes to ElevenLabs for transcription; transcript text goes to OpenAI (the server's current text model, which may be switched to Google Gemini or Anthropic Claude after updating this policy) for minutes, Q&A and translation, or to your own provider when you add a key; Cloudflare carries the traffic. The app writes to your Photos library only when you choose “Save video”. No ads, no tracking or analytics SDKs in the app, no sale of data, no model training on your content; the website counts page views with Cloudflare Web Analytics, which sets no cookies. Files and everything made from them are deleted automatically after 7 days (free) or 14 days (Plus); security logs are kept 180 days; database backups taken on the server before some updates are removed once the update is stable; after account deletion only anonymised cost figures remain. You can download, edit and delete your data, and delete your Scribe account in the app (account menu, “Xoá tài khoản”); your {idpName} account is deleted at auth.hoandat.com/account. Scribe is not for children under 13.",
	},
	terms: {
		title: "Điều khoản sử dụng",
		intro:
			"Khi dùng Scribe (trang web scribe.hoandat.com và ứng dụng Scribe trên iPhone), bạn đồng ý với các điều khoản dưới đây và với <privacy>Chính sách quyền riêng tư</privacy>.",
		account: {
			title: "Tài khoản",
			body: "Bạn đăng nhập bằng tài khoản {idpName} và chịu trách nhiệm giữ an toàn cho nó. Mỗi tài khoản dành cho một người. Bạn cần từ 16 tuổi trở lên, hoặc từ 13 tuổi và có sự đồng ý của cha mẹ hoặc người giám hộ.",
		},
		plans: {
			title: "Gói và hạn mức",
			body: "Gói miễn phí có 200 MB lưu trữ và giữ tệp 7 ngày; gói Plus có 5 GB và giữ tệp 14 ngày. Số phút chép lời và số lượt tóm tắt, hỏi đáp, dịch của gói hiện ở thẻ “Gói và mức dùng”. Hạn mức có thể thay đổi; khi hạn mức chung của dịch vụ đã hết, một số tính năng tạm dừng tới tháng sau.",
		},
		storage: {
			title: "Scribe không phải kho lưu trữ",
			body: "Tệp và mọi thứ đi kèm tự xoá khi hết hạn của gói. Bạn tự tải về và giữ bản cần dùng lâu dài; chúng tôi không khôi phục được dữ liệu đã xoá.",
		},
		content: {
			title: "Nội dung của bạn",
			body: "Bạn giữ mọi quyền với nội dung của mình. Bạn cho phép Scribe lưu, xử lý và gửi nội dung tới các bên xử lý nêu trong Chính sách quyền riêng tư, chỉ để cung cấp tính năng bạn dùng.",
		},
		use: {
			title: "Dùng đúng mục đích",
			body: "- Hỏi và có sự đồng ý của người được ghi âm, ghi hình theo quy định của pháp luật; không ghi lén.\n- Không tải lên nội dung trái pháp luật hay xâm phạm quyền của người khác.\n- Không tạo nhiều tài khoản để vượt hạn mức, không dò quét, tấn công hay làm quá tải dịch vụ.",
		},
		ai: {
			title: "Kết quả do máy tạo",
			body: "Lời chép, tên người nói, biên bản, bản dịch và câu trả lời của “Hỏi Scribe” do mô hình máy tạo ra và có thể sai. Hãy kiểm tra lại trước khi dùng cho việc quan trọng.",
		},
		keys: {
			title: "Key riêng",
			body: "Việc chạy bằng key của bạn tính phí vào tài khoản của bạn ở nhà cung cấp đó và theo điều khoản của họ. Bạn có thể xoá key bất cứ lúc nào.",
		},
		service: {
			title: "Dịch vụ",
			body: "Scribe được cung cấp như hiện có. Dịch vụ có thể gián đoạn, thay đổi hoặc ngừng; chúng tôi không bảo đảm dịch vụ luôn sẵn sàng hay không có lỗi. Trong phạm vi pháp luật cho phép, chúng tôi không chịu trách nhiệm cho thiệt hại gián tiếp hay việc mất dữ liệu đã hết hạn lưu.",
		},
		end: {
			title: "Chấm dứt",
			body: "Bạn có thể xoá tài khoản Scribe bất cứ lúc nào trong menu tài khoản. Chúng tôi có thể khoá tài khoản vi phạm các điều khoản này.",
		},
		law: {
			title: "Luật áp dụng",
			body: "Các điều khoản này theo pháp luật Việt Nam.",
		},
		changes: {
			title: "Thay đổi điều khoản",
			body: "Khi điều khoản thay đổi, ngày hiệu lực ở đầu trang đổi theo; thay đổi quan trọng được báo trong ứng dụng trước khi áp dụng.",
		},
		english:
			"By using Scribe you accept these terms and the Privacy Policy. Sign in with your {idpName} account and keep it safe; one account per person; you must be 16 or older, or 13 or older with a parent's consent. Free plan: 200 MB, files kept 7 days; Plus: 5 GB, 14 days; other limits are shown in the app and may change. Scribe is a workspace, not an archive: download what you need to keep. You own your content and let Scribe process it with the providers named in the Privacy Policy only to run the features you use. Get the consent of the people you record as the law requires; no unlawful content, no abuse. Transcripts, minutes, translations and answers are machine-made and may be wrong. Work run on your own API keys is billed by your provider. The service is provided as is and may change or stop. You can delete your Scribe account in the app at any time; accounts that break these terms may be blocked. Vietnamese law applies.",
	},
	contact: {
		title: "Liên hệ",
		body: "Câu hỏi hoặc yêu cầu về dữ liệu và các điều khoản này: <contact>{contact}</contact>.",
	},
} as const;
