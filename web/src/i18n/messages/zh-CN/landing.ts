import type { SourceMessages, Translation } from "../../message-types";

/** Simplified Chinese of `vi/landing.ts`: the same keys, the same facts. */
export const landing: Translation<SourceMessages["landing"]> = {
	nav: {
		features: "功能",
		how: "使用方法",
	},
	hero: {
		eyebrow: "录制并转写越南语",
		title: "开口说话，拿到文字稿。",
		lede: "Scribe 可直接在浏览器或 iPhone 上录制，把较长的音频和视频文件拆分成片段，再按发言人把越南语转写成文字，方便你搜索、编辑和下载。",
	},
	signIn: {
		title: "下次开会时试试看",
		body: "免费版提供 200 MB 存储空间；文件在 7 天后删除，需要保留的内容请及时下载。",
	},
	features: {
		eyebrow: "功能",
		title: "整场会议，一处搞定",
		record: {
			title: "在网页和 iPhone 上录制",
			body: "直接在浏览器中录制麦克风，或录制电脑的声音和屏幕。在 iPhone 上，锁屏后 Scribe 仍会继续录制。",
		},
		split: {
			title: "拆分音频和视频文件",
			body: "上传较长的文件，按分钟数、按大小或平均拆分，然后将片段打包为 zip 下载。",
		},
		transcribe: {
			title: "越南语文字稿",
			body: "文字稿按发言人分段，带有时间点方便回听。可修改句子、为发言人命名，并导出为 TXT、SRT、VTT、Markdown 或 Word。",
		},
		after: {
			title: "会议纪要、搜索、问答",
			body: "把内容总结为会议纪要，在所有文字稿中搜索，向 Scribe 提问并获得带时间点的回答，还可在越南语和英语之间翻译文字稿。",
		},
	},
	how: {
		eyebrow: "使用方法",
		title: "三步拿到文字稿",
		steps: {
			signIn: {
				title: "登录",
				body: "使用你的 {idpName} 账号，无需新密码。",
			},
			capture: {
				title: "录制或上传",
				body: "在网页或 iPhone 上录制，或选择已有的音频或视频文件。",
			},
			read: {
				title: "拿到文字稿",
				body: "点击“转写”，完成后即可阅读、编辑、搜索和下载。",
			},
		},
	},
};
