import type { SourceMessages, Translation } from "../../message-types";

/** Simplified Chinese of `vi/ui.ts`. */
export const ui: Translation<SourceMessages["ui"]> = {
	loading: "正在加载…",
	usage: {
		region: "用量",
		retention: { other: "文件在 {count} 天后删除" },
		storage: "存储空间",
		storageMeter: "已用存储空间",
		ownKey: {
			transcribe: "转写",
			text: "会议纪要、问答、翻译",
			value: "使用你的密钥（{provider}）",
			minutes: { other: "本月使用你的密钥转写了 {count} 分钟" },
		},
		storageFull: "存储空间已满：请删除部分录音或文件后再继续录制和上传。",
	},
	player: {
		playFailed: "无法播放：{error}",
		loadingRecording: "正在加载录音以播放…",
		loadingAudio: "正在加载音频…",
		play: "播放",
		pause: "暂停",
		seek: "跳转",
		mute: "静音",
		unmute: "取消静音",
		volume: "音量",
	},
	toast: {
		region: "通知",
		close: "关闭通知",
	},
};
