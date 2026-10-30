import type { SourceMessages, Translation } from "../../message-types";

/** Simplified Chinese of `vi/layout.ts`. */
export const layout: Translation<SourceMessages["layout"]> = {
	skipLink: "跳到主要内容",
	breadcrumb: "面包屑导航",
	appHeader: {
		home: "Scribe，前往录制",
		nav: "功能",
		record: "录制",
		split: "拆分文件",
		transcript: "文字稿",
	},
	idpMenu: {
		account: "{idpName} 账号",
		apps: "应用",
		opensIdp: "，在新标签页中打开 {idpName} 页面",
	},
	signOutConfirm: {
		title: "退出登录",
		question: "工作还没完成，确定要退出登录吗？",
		recording: "正在进行的录制将停止；已录制的内容会保留。",
		segmentsWaiting: {
			other:
				"{count} 个尚未上传的片段会保留在此设备上，并在你重新登录后继续上传。",
		},
		fileUploading: "“拆分文件”中正在上传的文件将中途停止。",
		confirm: "仍然退出登录",
	},
	footer: {
		links: "账号链接",
		ecosystem: "hoandat.com 旗下应用",
	},
	leaveGuard: {
		title: "离开页面",
		question: "尚未保存，确定要离开吗？",
		detail: "未保存的修改将会丢失。",
		confirm: "仍然离开",
	},
	pageError: {
		title: "此页面出错了",
		body: "正在进行的录制（如有）会继续。请打开其他页面或重新加载此页面。",
		reload: "重新加载页面",
	},
};
