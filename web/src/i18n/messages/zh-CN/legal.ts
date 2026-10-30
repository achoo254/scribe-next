import type { SourceMessages, Translation } from "../../message-types";

/**
 * Simplified Chinese of `vi/legal.ts`: a full translation of the privacy
 * policy and the terms. The Vietnamese text is the one that applies
 * (`authoritative`, shown at the top of the Chinese page). App labels
 * quoted in the text are the Chinese UI's. `english*` is the short English
 * summary the Vietnamese page ends with; the Chinese page does not show
 * it, so it stays English.
 */
export const legal: Translation<SourceMessages["legal"]> = {
	effective: "自 {date} 起生效",
	nav: {
		label: "法律文件",
		privacy: "隐私",
		terms: "条款",
	},
	contactFallback: "通过你的 {idpName} 账号联系 Scribe 运营者",
	englishTitle: "英文摘要",
	authoritative: {
		title: "关于本译文",
		body: "本中文文本是为方便你阅读而提供的译文。适用的是越南语版本；如两者有出入，以越南语版本为准。",
	},
	privacy: {
		title: "隐私政策",
		intro:
			"Scribe（网站 scribe.hoandat.com 以及 iPhone 上的 Scribe 应用）用于录制、拆分文件并将会议中的越南语转写成文字。本政策说明 Scribe 收集哪些数据、用于什么目的、发送给谁、保留多久以及你享有哪些权利。“我们”指 Scribe 的运营者。",
		collect: {
			title: "我们收集的数据",
			body: "- <b>账号：</b>你登录时 auth.hoandat.com 发送给 Scribe 的 {idpName} 账号邮箱、显示名称、头像（图片链接）和账号标识符；你当前的套餐。\n- <b>你创建的内容：</b>录音、屏幕录制、你上传的文件、文字稿、你修改的句子、发言人名称、标题、会议纪要、标记和译文。\n- <b>“问 Scribe”的问题：</b>仅用于回答，回答后即丢弃，不会保存。\n- <b>你的 API 密钥：</b>你自行输入的 API 密钥，以加密方式（AES-256-GCM）保存；Scribe 只显示最后 4 个字符，绝不会把密钥发回浏览器或应用。\n- <b>用量：</b>存储空间、转写分钟数，以及会议纪要、提问和翻译的次数。\n- <b>安全日志：</b>时间、操作（登录、上传、删除…）和 IP 地址；服务器的技术访问日志。\n- <b>在 iPhone 上：</b>录制的片段会保留在设备上，直到上传完成；设备的登录令牌保存在钥匙串（Keychain）中。应用仅在你录制时使用麦克风，仅在你选择拍摄或挑选文件上传时使用相机和照片图库，仅在你在共享菜单中选择“存储视频”时写入照片图库；应用不会读取你的通讯录或位置。\n\nScribe 没有广告，应用中不使用任何跟踪或分析 SDK，不出售数据，也不会在其他应用或网站上跟踪你。网站 scribe.hoandat.com 使用 Cloudflare Web Analytics 统计页面浏览量；它不设置 Cookie，也不在你的设备上存储任何内容。",
		},
		purposes: {
			title: "我们如何使用数据",
			body: "- 提供你所使用的功能：录制、存储、播放、拆分、转写、实时转写、会议纪要、搜索、问答、翻译、导出文件以及把会议纪要发送到 chat。\n- 执行你套餐的额度限制并防止滥用。\n- 保障你的账号安全并处理事故。\n\n我们不会使用你的内容训练模型，也不会查看你的内容，除非你就某条录音请求支持。",
		},
		processors: {
			title: "数据处理方",
			body: "数据只会发送给以下各方，且仅用于完成你要求的操作：\n\n- <b>Scribe 的服务器</b>（位于越南的租用服务器）：存储文件、文字稿和数据库。\n- <b>Cloudflare</b>：转发流量，并在你的设备与服务器之间提供 HTTPS 加密。<cloudflare>Cloudflare 的政策</cloudflare>。\n- <b>ElevenLabs</b>：在你点击“转写”时接收音频，在你于 iPhone 上开启“实时转写”时接收实时音频。<elevenlabs>ElevenLabs 的政策</elevenlabs>。\n- <b>OpenAI</b>：目前是服务器的文本模型服务商。在你点击“生成纪要”“问 Scribe”或“翻译”时接收文字稿文本；当服务器使用 OpenAI 作为备用转写途径时接收音频。<openai>OpenAI 的政策</openai>。服务器可能被配置为使用 Google Gemini 或 Anthropic Claude 代替 OpenAI 完成这些工作；如有此变更，会先更新本政策。\n- <b>你的 API 密钥所属服务商：</b>当你添加自己的密钥（OpenAI、Google Gemini、Anthropic Claude 或 ElevenLabs）时，相应的工作会在你于该服务商的账号上运行，并适用你与其约定的条款。<google>Google 的政策</google>、<anthropic>Anthropic 的政策</anthropic>。\n- <b>你的 {idpName} 账号</b>（auth.hoandat.com）：登录。\n- <b>chat.hoandat.com</b>：仅在你关联 chat 时使用；它接收你选择发送的会议纪要。当某个账号多次达到免费额度上限时，该账号的邮箱会被发送到运营者在 chat.hoandat.com 上的私人提醒频道。",
		},
		retention: {
			title: "数据保留期限",
			body: "- <b>文件及由其生成的一切内容</b>（文字稿、修改过的句子、会议纪要、标记、译文、搜索索引）：免费版在创建 7 天后自动删除，Plus 版在创建 14 天后自动删除。Scribe 是临时工作区，而不是存档：需要保留的内容请下载。\n- <b>账号、你的 API 密钥、chat 关联：</b>保留至你删除为止。\n- <b>安全日志：</b>180 天。\n- <b>每月用量数据：</b>保留至你删除账号为止。删除后仅保留匿名的费用数据（不再与你或任何录音相关联），用于核对服务成本。\n- <b>备份：</b>在部分服务器更新之前，运营者会把数据库（不含音频文件）复制到同一服务器上的备份文件夹，以便出问题时恢复；更新稳定后这些副本即被删除。你已删除的数据在这类备份被删除之前可能仍留存其中。\n- 各数据处理方按其自身政策保留数据。",
		},
		rights: {
			title: "你的权利",
			body: "- <b>查看和下载：</b>播放并下载每条录音，在“录音记录”中使用“全部下载”，将文字稿导出为 TXT、SRT、VTT、Markdown 或 Word。\n- <b>编辑：</b>修改句子、标题和发言人名称。\n- <b>删除：</b>删除单条录音、“全部删除”、删除你的 API 密钥、取消关联 chat。\n- <b>删除账号：</b>在账号菜单中选择“删除账号”。Scribe 会立即删除你的所有文件、文字稿、会议纪要、译文、标记、搜索索引、API 密钥、chat 关联、登录会话和用量数据。安全日志会保留至 180 天期满，但不再包含你的邮箱、文件名或错误详情；费用数据以匿名形式保留。你的 {idpName} 账号由多个应用共用；请在 <account>auth.hoandat.com/account</account> 删除。\n- 其他数据请求（例如获取你账号数据的副本）：请按“联系方式”中的说明联系我们。",
		},
		security: {
			title: "安全",
			body: "连接使用 HTTPS；登录会话使用 HttpOnly Cookie 且可以撤销；你的 API 密钥经过加密；每个账号只能看到自己的数据。没有任何系统是绝对安全的；如果我们发现影响你数据的事故，会告知你。",
		},
		children: {
			title: "儿童",
			body: "Scribe 不面向 13 岁以下的儿童，也不会在知情的情况下收集 13 岁以下儿童的数据。13 岁至未满 16 岁的用户须经父母或监护人同意方可使用。如果你知道有儿童使用了 Scribe，请联系我们以便删除相关数据。",
		},
		changes: {
			title: "本政策的变更",
			body: "政策变更时，页面顶部的生效日期会随之更新；重要变更会在生效前于应用内通知。",
		},
		english:
			"Scribe records meetings and transcribes Vietnamese. We collect your {idpName} account email, name and profile picture link, the recordings and files you create, their transcripts and the things you make from them (edits, minutes, bookmarks, translations), your own API keys (encrypted, never shown back), usage counts, and security logs (time, action, IP address). Audio goes to ElevenLabs for transcription; transcript text goes to OpenAI (the server's current text model, which may be switched to Google Gemini or Anthropic Claude after updating this policy) for minutes, Q&A and translation, or to your own provider when you add a key; Cloudflare carries the traffic. The app writes to your Photos library only when you choose “Save Video”. No ads, no tracking or analytics SDKs in the app, no sale of data, no model training on your content; the website counts page views with Cloudflare Web Analytics, which sets no cookies. Files and everything made from them are deleted automatically after 7 days (free) or 14 days (Plus); security logs are kept 180 days; database backups taken on the server before some updates are removed once the update is stable; after account deletion only anonymised cost figures remain. You can download, edit and delete your data, and delete your Scribe account in the app (account menu, “Delete account”); your {idpName} account is deleted at auth.hoandat.com/account. Scribe is not for children under 13.",
	},
	terms: {
		title: "使用条款",
		intro:
			"使用 Scribe（网站 scribe.hoandat.com 以及 iPhone 上的 Scribe 应用）即表示你同意以下条款和<privacy>隐私政策</privacy>。",
		account: {
			title: "账号",
			body: "你使用 {idpName} 账号登录，并负责保障其安全。每个账号仅限一人使用。你必须年满 16 岁，或年满 13 岁并经父母或监护人同意。",
		},
		plans: {
			title: "套餐与额度",
			body: "免费版提供 200 MB 存储空间，文件保留 7 天；Plus 版提供 5 GB，文件保留 14 天。你套餐的转写分钟数以及会议纪要、提问和翻译次数显示在“套餐与用量”卡片上。额度可能会调整；当系统的共享额度用完时，部分功能会暂停至下个月。",
		},
		storage: {
			title: "Scribe 不是存档",
			body: "文件及由其生成的一切内容会在你套餐的保留期结束时自动删除。需要长期保存的内容请自行下载保存；已删除的数据我们无法恢复。",
		},
		content: {
			title: "你的内容",
			body: "你保留对自己内容的全部权利。你允许 Scribe 存储和处理你的内容，并将其发送给隐私政策中列明的数据处理方，仅用于提供你所使用的功能。",
		},
		use: {
			title: "合理使用",
			body: "- 按照法律要求，征得并取得被你录音或录像者的同意；不得偷录。\n- 不得上传违法内容或侵犯他人权利的内容。\n- 不得创建多个账号以规避额度限制，不得扫描、攻击服务或使其过载。",
		},
		ai: {
			title: "机器生成的结果",
			body: "文字稿、发言人名称、会议纪要、译文以及“问 Scribe”的回答均由机器模型生成，可能有误。用于重要事项前请先核对。",
		},
		keys: {
			title: "你的 API 密钥",
			body: "使用你的密钥完成的工作由相应服务商按其条款向你的账号收费。你可以随时删除自己的密钥。",
		},
		service: {
			title: "服务",
			body: "Scribe 按现状提供。服务可能会中断、变更或终止；我们不保证服务始终可用或没有错误。在法律允许的范围内，我们不对间接损失或保留期已结束的数据丢失承担责任。",
		},
		end: {
			title: "终止",
			body: "你可以随时通过账号菜单删除 Scribe 账号。对于违反本条款的账号，我们可能会予以封禁。",
		},
		law: {
			title: "适用法律",
			body: "本条款受越南法律管辖。",
		},
		changes: {
			title: "本条款的变更",
			body: "条款变更时，页面顶部的生效日期会随之更新；重要变更会在生效前于应用内通知。",
		},
		english:
			"By using Scribe you accept these terms and the Privacy Policy. Sign in with your {idpName} account and keep it safe; one account per person; you must be 16 or older, or 13 or older with a parent's consent. Free plan: 200 MB, files kept 7 days; Plus: 5 GB, 14 days; other limits are shown in the app and may change. Scribe is a workspace, not an archive: download what you need to keep. You own your content and let Scribe process it with the providers named in the Privacy Policy only to run the features you use. Get the consent of the people you record as the law requires; no unlawful content, no abuse. Transcripts, minutes, translations and answers are machine-made and may be wrong. Work run on your own API keys is billed by your provider. The service is provided as is and may change or stop. You can delete your Scribe account in the app at any time; accounts that break these terms may be blocked. Vietnamese law applies.",
	},
	contact: {
		title: "联系方式",
		body: "关于你的数据和本条款的问题或请求：<contact>{contact}</contact>。",
	},
};
