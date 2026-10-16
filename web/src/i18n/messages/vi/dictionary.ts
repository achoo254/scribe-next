import { common } from "./common";
import { ui } from "./ui";

/**
 * Vietnamese, the source dictionary: its literal types define every key
 * (`MessageKey`) and the placeholders each text takes. A later language
 * is typed `Messages` and must fill exactly these keys.
 */
export const vi = {
	common,
	ui,
} as const;
