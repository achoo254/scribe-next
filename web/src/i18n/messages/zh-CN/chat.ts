import type { SourceMessages, Translation } from "../../message-types";

/**
 * Simplified Chinese of `vi/chat.ts`. The Scribe server words the bot's
 * linking steps in the language the link was made in, so step 3 names the
 * Chinese button.
 */
export const chat: Translation<SourceMessages["chat"]> = {
	menu: {
		link: "关联 chat.hoandat.com",
		unlink: "取消关联 chat",
		unlinked: "已取消关联 chat",
		unlinkFailed: "无法取消关联 chat",
		unlinkFrom: "取消关联 chat（{name}）",
	},
	dialog: {
		title: "关联 chat.hoandat.com",
		intro:
			"只需关联一次，Scribe 即可把会议纪要发送到你在 chat.hoandat.com 上与 Scribe 机器人的私聊中。",
		step1: "点击“打开 chat”（浏览器会打开 chat.hoandat.com）。",
		step2: "如有提示请先登录，然后在与 Scribe 机器人的聊天中发送 /start。",
		step3: "机器人会回复你的邮箱：点击“确认关联”。",
		step4: "回到这里：关联完成后 Scribe 会自动识别。",
		steps: "关联步骤",
		expires: "该链接只能使用一次，将于 {time} 失效。",
		preparing: "正在创建关联链接…",
		open: "打开 chat",
		opensBrowser: "，在浏览器中打开",
		waiting: "正在等待你在 chat 中发送 /start 并确认关联…",
		linked: "已关联。会议纪要将发送到你与 Scribe 机器人的私聊中。",
		linkedTitle: "已关联 chat",
	},
};
