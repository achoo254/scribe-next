import type { SourceMessages, Translation } from "../../message-types";

/**
 * Simplified Chinese of `vi/recorder.ts`. Settings paths follow iOS in
 * Simplified Chinese.
 */
export const recorder: Translation<SourceMessages["recorder"]> = {
	defaultName: "录音 {date}",
	micHint: {
		iosEdge: "前往 iOS“设置”→ Edge →“麦克风”→ 开启，然后重新加载页面再试。",
		iosChrome: "前往 iOS“设置”→ Chrome →“麦克风”→ 开启，然后重新加载页面再试。",
		iosFirefox:
			"前往 iOS“设置”→ Firefox →“麦克风”→ 开启，然后重新加载页面再试。",
		iosSafari:
			"轻点地址栏左侧的“aA”→“网站设置”→“麦克风”→“允许”（或前往 iOS“设置”→ Safari 浏览器 →“麦克风”），然后重新加载页面。",
		desktop:
			"点击地址栏左侧的锁形/权限图标 → 允许“麦克风”，然后重新加载页面再试。",
	},
	startError: {
		screenDenied:
			"未获得屏幕共享/麦克风权限。请选择要共享的内容并允许访问，然后重试。",
		micDenied: "浏览器没有麦克风权限。{hint}",
		notFound: "未找到可用的麦克风。",
		busy: "其他应用正在使用麦克风。请关闭该应用后重试。",
		aborted: "无法访问麦克风（设备被占用或被中断）。请重试。",
		insecure: "录制需要安全连接（HTTPS）。",
		failed: "无法开始录制。",
		denied: "无法开始录制（访问被拒绝？）",
	},
	pauseError: "无法暂停或继续。",
	interrupt: {
		micEnded: "录制已停止（麦克风断开，可能是因为锁屏）。已录制的内容已保存。",
		hidden: "锁屏或切换应用时录制已停止。已录制的内容已保存。",
	},
	storageFull: "{message} 录制已停止，截至此时的内容已保存。",
	chunksLost: "部分片段遇到网络错误。已上传的内容已保存（录音可能不完整）。",
	noSource: "无法获取任何来源。请检查麦克风/屏幕共享权限。",
	mediaError: "录制出错（MediaRecorder），已停止",
	invalidSession: "录制会话无效",
	finalizeFailed: "无法完成录制",
	chunkFailed: "分块上传失败",
	uploadFailed: "录音上传失败",
	native: {
		signInTitle: "登录已过期",
		signInBody: "请重新登录，以上传此设备上等待中的片段。",
		quotaTitle: "存储空间不足",
		quotaBody: "{message} 片段会保留在此设备上，有空间后再上传。",
		lostTitle: "录音结尾丢失",
		lostBody: "录制时应用被关闭：正在录制的片段最多丢失了 {seconds} 秒。",
		syncTitle: "无法同步此设备上录制的片段",
		syncRetry: "重新打开应用以重试。",
	},
};
