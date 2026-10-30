import type { Messages } from "../../message-types";
import { account } from "./account";
import { chat } from "./chat";
import { common } from "./common";
import { errors } from "./errors";
import { keys } from "./keys";
import { landing } from "./landing";
import { layout } from "./layout";
import { legal } from "./legal";
import { login } from "./login";
import { record } from "./record";
import { recorder } from "./recorder";
import { search } from "./search";
import { split } from "./split";
import { transcript } from "./transcript";
import { ui } from "./ui";

/**
 * Simplified Chinese: every key of the Vietnamese source, signed in or
 * not. The legal texts are a full translation; their Vietnamese version is
 * the one that applies (`legal.authoritative`).
 */
export const zhCN: Messages = {
	account,
	chat,
	common,
	errors,
	keys,
	landing,
	layout,
	legal,
	login,
	record,
	recorder,
	search,
	split,
	transcript,
	ui,
};
