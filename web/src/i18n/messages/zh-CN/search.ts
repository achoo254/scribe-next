import type { SourceMessages, Translation } from "../../message-types";

/**
 * Simplified Chinese of `vi/search.ts`: search and “Ask Scribe”. The
 * examples in the search fields stay Vietnamese: the transcripts are
 * Vietnamese speech, and the first one shows a search without accents.
 */
export const search: Translation<SourceMessages["search"]> = {
	entry: {
		label: "搜索文字稿",
		placeholder: "搜索句子，例如：hop dong",
		submit: "搜索",
	},
	page: {
		back: "文字稿",
		title: "搜索录音",
		description:
			"在你所有的文字稿中查找句子；越南语带不带声调符号均可。点击句子即可从该处开始播放。",
		field: "搜索文字稿",
		placeholder: "例如：hợp đồng thuê nhà",
	},
	results: {
		title: "结果",
		count: { other: "{count} 个匹配的句子" },
		showing: "{total} 个结果中最匹配的 {count} 个",
		searching: "正在搜索…",
		idle: "输入几个词即可搜索你的文字稿。",
		tooShort: "请至少输入 2 个字符进行搜索。",
		emptyTitle: "没有匹配的句子",
		emptyBody: "请换个词或减少词数。仅搜索已转写的句子。",
		failed: "搜索失败",
	},
	ask: {
		title: "问 Scribe",
		introAll:
			"用自然语言就你的所有录音提问。回答仅依据录音内容，并引用时间点方便回听。",
		introOne:
			"用自然语言就这条录音提问。回答仅依据录音内容，并引用时间点方便回听。",
		field: "向 Scribe 提问",
		placeholderAll: "例如：客户同意的价格是多少？",
		placeholderOne: "例如：谁来发送报价？",
		submit: "提问",
		running: "正在查找答案…可能需要几十秒。",
		failed: "无法回答",
		question: "你的问题：{question}",
		sources: "来源",
		cite: "来源 {n}：{title}，从 {time} 开始播放",
		session: "问题仅保留到你离开此页面为止。",
		dailyLimit: "你今天已提问 {used}/{limit} 次。将于 {time} 恢复。",
		monthlyLimit:
			"此账号本月的会议纪要和提问额度已用完（{used}/{limit} 等效分钟）。将于 {time} 恢复。",
		capReached: "提问已用完本月的共享额度。将于 {time} 恢复。",
	},
};
