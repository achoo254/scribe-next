import type { SourceMessages, Translation } from "../../message-types";

/** Simplified Chinese of `vi/record.ts`: the Record page and the recording history. */
export const record: Translation<SourceMessages["record"]> = {
	title: "录制",
	description:
		"将麦克风、系统声音或屏幕录制为一条录音。录制时会同步上传，并显示在下方的“录音记录”中。",
	nativeDescription:
		"使用 iPhone 麦克风录制，锁屏或来电时也不会中断。每个录完的片段会在继续录制的同时上传到“录音记录”。",
	noSource: "请至少开启一个来源。",
	sourcesHelp:
		"录制系统声音和屏幕需要共享屏幕，在 <b>Chrome/Edge</b> 中效果最佳：选择“整个屏幕”并开启“分享音频”即可录到系统声音。Firefox/Safari 可能不支持系统声音。多个来源 → 混合为一条录音。",
	noScreenCapture: "此浏览器无法共享屏幕（仅支持麦克风）。",
	sources: {
		legend: "来源",
		mic: "麦克风",
		systemAudio: "系统声音",
		screen: "包含屏幕（视频）",
	},
	upload: {
		choose: "上传文件",
		hint: "选择设备上的音频或视频文件，用于播放和转写。",
		progress: "上传进度",
		uploading: "正在上传“{name}”… {percent}%",
		resume: "继续上传",
		done: "已上传“{name}”。可在下方的<b>录音记录</b>中播放和转写。",
	},
	splitAfter: {
		toggle: "按分钟数拆分为片段",
		minutes: "每个片段的分钟数",
		invalid: "请输入大于 0 的分钟数。",
	},
	live: {
		level: "音量电平",
		monitor: "整个屏幕",
		window: "窗口",
		browser: "浏览器标签页",
	},
	start: "开始录制",
	stop: "停止录制",
	pause: "暂停",
	resume: "继续",
	timer: "正在录制 — {clock}",
	timerPaused: "已暂停 — {clock}",
	saving: "正在保存录音…",
	splitting: "正在将录音拆分为片段…",
	splitDone: {
		other: "已拆分为 {count} 个片段。请在下方的<b>录音记录</b>中查看。",
	},
	bookmark: {
		mark: "标记",
		marked: { other: "{count} 个标记" },
		waiting: { other: "{count} 个标记等待保存，正在重试" },
		failed: { other: "{count} 个标记无法保存：{reason}" },
		dropped: { other: "{count} 个标记因录音已被拆分或删除而丢弃" },
		unknownReason: "原因不明",
	},
	native: {
		segmentLength: "片段时长",
		segmentOption: { other: "{count} 分钟" },
		uploadsWaiting: {
			other: "等待上传：{count} 个片段（保存在此设备上，联网后上传）。",
		},
		uploadsFailed: {
			other: "{count} 个片段被服务器拒绝，已保留在此设备上。",
		},
		realtime: "实时转写",
		realtimeHint:
			"将音频发送到转写服务，每句话说完后即可显示文字；计入你的转写分钟数。上传后仍会进行完整转写。",
		liveTitle: "正在转写",
		liveTitleDone: "实时转写",
		liveNote: "预览。上传后会进行完整转写。",
		liveConnecting: "正在连接…",
		liveEmpty: "每句话说完后，文字会显示在这里。",
		liveDisconnected: "自 {clock} 起连接中断，正在重新连接…",
		liveGap: "{from}–{to} · 已漏掉，请等待完整转写",
	},
	groups: {
		expiresSoon: "即将删除",
		expiresIn: { other: "{count} 天后删除" },
		iphoneSession: "iPhone 录音",
		today: "今天",
		yesterday: "昨天",
	},
	history: {
		title: "录音记录",
		downloadAll: "全部下载",
		loading: "正在加载录音记录…",
		loadFailed: "无法加载录音记录",
		emptyTitle: "还没有录音",
		emptyBody: "点击“停止录制”后，录音会显示在这里。",
		retention: {
			other: "录音会在创建 <b>{count} 天后自动删除</b>。如需长期保留，请下载。",
		},
		dayMeta: { other: "{count} 条录音 · {duration}" },
		downloadDay: "下载当天",
		downloadDayLabel: "下载 {date} 的录音",
		deleted: "已删除“{name}”",
		undo: "撤销",
		deleteAll: "全部删除",
		deleteAllTitle: "删除全部录音记录",
		deleteAllQuestion: { other: "删除 {count} 个文件，共 {size}？" },
		deleteAllBody:
			"此列表中的所有录音和上传的文件，连同其文字稿、会议纪要和标记，都将被永久删除。此操作无法撤销。保留：正在进行的录音（最近 10 分钟内有新片段的 iPhone 录音视为正在进行）以及正在拆分的录音。",
		deletedAll: { other: "已删除 {count} 个文件" },
		deletedAllSkipped: {
			other: "已保留 {count} 个属于正在进行或正在拆分的录音的文件。",
		},
	},
	row: {
		screen: "屏幕",
		mic: "麦克风",
		upload: "上传的文件",
		delete: "删除 {name}",
		play: "播放",
		hide: "隐藏",
		menu: "{name} 的操作",
		download: "下载",
		saveToFiles: "存储到“文件”",
	},
	rename: {
		edit: "重命名 {name}",
		field: "录音名称",
		hint: "按 Enter 保存，按 Esc 取消。留空则使用默认名称。",
	},
	session: {
		badge: "iPhone",
		meta: { other: "{count} 个片段 · {duration} · {size}" },
		expand: "显示片段",
		collapse: "收起",
		part: "片段 {number}",
		delete: "删除整段录音 {name}",
	},
};
