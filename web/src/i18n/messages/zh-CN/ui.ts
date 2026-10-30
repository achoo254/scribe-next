import type { SourceMessages, Translation } from "../../message-types";

/** Simplified Chinese of `vi/ui.ts`. */
export const ui: Translation<SourceMessages["ui"]> = {
	loading: "正在加载…",
	usage: {
		region: "套餐与用量",
		plan: { free: "免费版", plus: "Plus 版" },
		planUntil: "有效期至 {date}",
		retention: { other: "文件在 {count} 天后删除" },
		resets: "分钟数于 {date} 重置",
		storage: "存储空间",
		storageMeter: "已用存储空间",
		month: "本月转写",
		monthMeter: "本月已用转写分钟数",
		today: "今天",
		todayMeter: "今天已用转写分钟数",
		minutes: "{used}/{limit} 分钟",
		ownKey: {
			transcribe: "转写",
			text: "会议纪要、问答、翻译",
			value: "使用你的密钥（{provider}）",
			minutes: { other: "本月使用你的密钥转写了 {count} 分钟" },
		},
		sharedLow: {
			other:
				"距离续期，系统的共享转写时长仅剩约 {count} 分钟，可能会先于你套餐的分钟数用完。",
		},
		sharedOut:
			"系统的共享转写时长已用完，将于 {date} 恢复。你套餐的分钟数仍会保留。",
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
