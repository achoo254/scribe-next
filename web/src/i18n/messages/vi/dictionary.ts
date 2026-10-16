import { common } from "./common";
import { layout } from "./layout";
import { login } from "./login";
import { record } from "./record";
import { split } from "./split";
import { transcript } from "./transcript";
import { ui } from "./ui";

/**
 * Vietnamese, the source dictionary: its literal types define every key
 * (`MessageKey`) and the placeholders each text takes. A later language
 * is typed `Messages` and must fill exactly these keys.
 */
export const vi = {
	common,
	layout,
	login,
	record,
	split,
	transcript,
	ui,
} as const;
