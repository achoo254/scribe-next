import { createRequire } from "node:module";

/**
 * The design tokens Scribe loads (src/styles/global.css): the copy in
 * @achoo254/app-shell, the one source of the tokens of every hoandat.com
 * app. Colour literals may live only there.
 */
export const TOKENS_FILE = createRequire(import.meta.url).resolve(
	"@achoo254/app-shell/tokens.css",
);
