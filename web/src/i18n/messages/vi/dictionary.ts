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
 * Vietnamese, the source dictionary: its literal types define every key
 * (`MessageKey`) and the placeholders each text takes. A later language
 * is typed `Messages` and must fill exactly these keys.
 */
export const vi = {
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
} as const;
