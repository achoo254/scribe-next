import type { SourceMessages, Translation } from "../../message-types";

/** Simplified Chinese of `vi/keys.ts`: the user's own API keys. */
export const keys: Translation<SourceMessages["keys"]> = {
	menu: "你的 API 密钥",
	dialog: {
		title: "你的 API 密钥",
		intro:
			"使用你自己的 API 密钥代替 Scribe 的密钥。密钥在 Scribe 服务器上加密保存，之后不会再显示；费用由服务商向你的账号收取，使用你的密钥完成的工作不计入 Scribe 的分钟数或次数。",
		unavailable:
			"此服务器未启用“你的 API 密钥”。需要管理员创建加密密钥文件并重启。",
		loading: "正在加载你的 API 密钥…",
		saved: "••••{last4} · {date} 已检查",
		none: "尚无密钥",
		status: {
			invalid: "服务商拒绝了这个密钥：请输入新密钥或将其删除。",
			quota: "该密钥所属账号余额不足：请充值或输入其他密钥。",
			unreadable: "服务器无法读取已保存的密钥：请重新输入。",
		},
		field: "{provider} 密钥",
		placeholderNew: "粘贴密钥",
		placeholderReplace: "粘贴新密钥以替换",
		save: "保存并检查",
		delete: "删除密钥",
		savedToast: "{provider} 密钥已检查并保存",
		deletedToast: "已删除 {provider} 密钥",
		create: "在 {provider} 创建密钥",
		opensBrowser: "，在新标签页中打开",
	},
	provider: {
		elevenlabs: {
			name: "ElevenLabs",
			usedFor:
				"转写文件和整段 iPhone 录音（区分发言人），实时转写。密钥需要 Speech to Text 权限。",
		},
		openai: {
			name: "OpenAI",
			usedFor:
				"会议纪要、问答、翻译；转写文件（gpt-transcribe，不区分发言人）；实时转写。",
		},
		gemini: {
			name: "Google Gemini",
			usedFor:
				"会议纪要、问答、翻译；转写文件（不区分发言人，句子时间为估算值）。不用于实时转写。",
		},
		anthropic: {
			name: "Anthropic Claude",
			usedFor: "会议纪要、问答、翻译。Claude 不做转写。",
		},
	},
	uses: {
		title: "各项工作使用哪个密钥",
		hint: "只能选择拥有有效密钥的服务商。",
		transcribe: "转写文件",
		live: "iPhone 上的实时转写",
		text: "会议纪要、问答、翻译",
		scribe: "使用 Scribe 的密钥，计入你套餐的用量。",
		needsKey: "{provider}（尚无密钥）",
		model: "模型",
		geminiNote: "Gemini：句子时间为估算值，不区分发言人。",
	},
};
