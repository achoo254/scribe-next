import type { SourceMessages, Translation } from "../../message-types";

/** Simplified Chinese of `vi/account.ts`. */
export const account: Translation<SourceMessages["account"]> = {
	menu: "删除账号",
	delete: {
		title: "删除 Scribe 账号",
		question: "要永久删除 {email} 的 Scribe 账号吗？",
		intro: "Scribe 会立即从服务器上删除：",
		files: "所有录音、上传的文件和拆分片段。",
		transcripts:
			"文字稿、修改过的句子、发言人名称、会议纪要、标记、译文和搜索索引。",
		keys: "你的 API 密钥以及与 chat.hoandat.com 的关联。",
		sessions: "所有设备上的登录会话以及账号的用量数据。",
		undo: "此操作无法撤销。请先下载需要保留的内容。",
		audit:
			"安全日志（时间、操作、IP 地址）最多保留 180 天，但不再包含你的邮箱或文件名；费用数据以匿名形式保留。",
		idp: "你的 {idpName} 账号由多个应用共用，不会在这里删除。如需一并删除，请前往 <account>auth.hoandat.com/account</account>。",
		busy: "正在录制或仍有文件在上传。请停止录制并等待上传完成后再删除账号。",
		confirmWord: "删除",
		confirmLabel: "输入 {word} 以确认",
		confirm: "删除账号",
		recordingOpen: "有录音正在进行。请停止录制后重试。",
	},
};
