import type { SourceMessages, Translation } from "../../message-types";

/** Simplified Chinese of `vi/login.ts`. */
export const login: Translation<SourceMessages["login"]> = {
	title: "登录",
	subtitle: "使用你的 {idpName} 账号录制、拆分文件和转写。",
	sessionCheck: "正在检查登录状态…",
	noSession: "已登录，但服务器尚未收到会话。请重试。",
	errors: {
		denied: "你在账号页面拒绝了登录。",
		expired: "登录已过期。请重新点击“登录”。",
		invalid: "登录响应无效。请重新点击“登录”。",
		unverified:
			"该账号的邮箱尚未验证。请使用已验证邮箱的 Google、GitHub 或 Apple 账号登录。",
		forbidden: "此账号暂时无法使用 Scribe。",
		deleting: "此 Scribe 账号刚被删除，数据正在清理中。请过几分钟再尝试登录。",
		idp: "登录服务器报告了错误。请稍后重试。",
		busy: "登录次数过多。请等待一分钟后重试。",
		failed: "登录失败。请稍后重试。",
	},
	failed: "登录失败",
	accountDeleted:
		"你的 Scribe 账号及所有数据已删除。你的 {idpName} 账号仍保留在 auth.hoandat.com。",
	accountDeletedPending:
		"你的 Scribe 账号已删除。其余数据将在一小时内删除完毕。",
};
