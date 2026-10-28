import type { Messages } from "../../message-types";
import { vi } from "../vi/dictionary";
import { landing } from "./landing";
import { login } from "./login";

/**
 * English: the signed-out pages only (landing, sign-in, the skip link of
 * their layout), where the shared shell offers the language switch. Every
 * other namespace is the Vietnamese one: the signed-in app and the legal
 * texts are Vietnamese whatever the visitor picked (the legal pages carry
 * their own English summary).
 */
export const en: Messages = {
	...vi,
	landing,
	login,
	layout: { ...vi.layout, skipLink: "Skip to main content" },
};
