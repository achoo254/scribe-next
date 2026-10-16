import { chat } from "./chat";
import { common } from "./common";
import { errors } from "./errors";
import { layout } from "./layout";
import { login } from "./login";
import { record } from "./record";
import { recorder } from "./recorder";
import { split } from "./split";
import { transcript } from "./transcript";
import { ui } from "./ui";

/**
 * Vietnamese, the source dictionary: its literal types define every key
 * (`MessageKey`) and the placeholders each text takes. A later language
 * is typed `Messages` and must fill exactly these keys.
 */
export const vi = {
	chat,
	common,
	errors,
	layout,
	login,
	record,
	recorder,
	split,
	transcript,
	ui,
} as const;
