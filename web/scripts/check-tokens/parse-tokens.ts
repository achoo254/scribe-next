/**
 * Minimal parser for `src/styles/tokens.css`. It only needs to
 * understand custom-property declarations grouped by their (possibly nested)
 * selector context, which is all the token file contains.
 */

export type TokenMap = Map<string, string>;

export interface TokenBlock {
	/** Selector/at-rule chain, outermost first, whitespace-normalised. */
	context: string[];
	tokens: TokenMap;
}

export interface ThemeTokens {
	light: TokenMap;
	dark: TokenMap;
	/** Dark overrides applied via `prefers-color-scheme` (block [2]). */
	darkAuto: TokenMap;
	/** Dark overrides applied via `data-theme="dark"` (block [3]). */
	darkForced: TokenMap;
}

const ROOT = ":root";
const DARK_MEDIA = "@media (prefers-color-scheme: dark)";
const DARK_AUTO_SELECTOR = ':root:not([data-theme="light"])';
const DARK_FORCED_SELECTOR = ':root[data-theme="dark"]';

function normalise(prelude: string): string {
	const collapsed = prelude
		.replace(/\s+/g, " ")
		.replace(/\(\s*/g, "(")
		.replace(/\s*\)/g, ")")
		.replace(/'/g, '"')
		.trim();
	// Media features are written either `(a:b)` or `(a: b)`; selectors such as
	// `:root:not(...)` must keep their colons untouched.
	return collapsed.startsWith("@")
		? collapsed.replace(/\s*:\s*/g, ": ")
		: collapsed;
}

function parseDeclarations(body: string): TokenMap {
	const tokens: TokenMap = new Map();
	for (const raw of body.split(";")) {
		const colon = raw.indexOf(":");
		if (colon === -1) continue;
		const name = raw.slice(0, colon).trim();
		if (!name.startsWith("--")) continue;
		tokens.set(name, raw.slice(colon + 1).trim());
	}
	return tokens;
}

/** Splits the stylesheet into blocks of custom properties with their context. */
export function parseTokenBlocks(css: string): TokenBlock[] {
	const source = css.replace(/\/\*[\s\S]*?\*\//g, "");
	const blocks: TokenBlock[] = [];
	const stack: string[] = [];
	const bodies: string[] = [];
	let buffer = "";

	for (const char of source) {
		if (char === "{") {
			stack.push(normalise(buffer));
			bodies.push("");
			buffer = "";
		} else if (char === "}") {
			const body = (bodies.pop() ?? "") + buffer;
			const tokens = parseDeclarations(body);
			if (tokens.size > 0) blocks.push({ context: [...stack], tokens });
			stack.pop();
			buffer = "";
		} else if (char === ";" && stack.length > 0) {
			bodies[bodies.length - 1] += `${buffer};`;
			buffer = "";
		} else {
			buffer += char;
		}
	}
	if (stack.length > 0) {
		throw new Error("tokens.css has an unclosed block");
	}
	return blocks;
}

function contextIs(block: TokenBlock, expected: string[]): boolean {
	return (
		block.context.length === expected.length &&
		block.context.every((part, index) => part === expected[index])
	);
}

function mergeInto(target: TokenMap, source: TokenMap): void {
	for (const [name, value] of source) target.set(name, value);
}

/** Resolves the light and dark token sets the browser would compute. */
export function resolveThemes(css: string): ThemeTokens {
	const blocks = parseTokenBlocks(css);
	const light: TokenMap = new Map();
	const darkAuto: TokenMap = new Map();
	const darkForced: TokenMap = new Map();

	for (const block of blocks) {
		if (contextIs(block, [ROOT])) mergeInto(light, block.tokens);
		if (contextIs(block, [DARK_MEDIA, DARK_AUTO_SELECTOR])) {
			mergeInto(darkAuto, block.tokens);
		}
		if (contextIs(block, [DARK_FORCED_SELECTOR])) {
			mergeInto(darkForced, block.tokens);
		}
	}
	if (light.size === 0) throw new Error("tokens.css has no :root block");
	if (darkForced.size === 0) {
		throw new Error(`tokens.css has no ${DARK_FORCED_SELECTOR} block`);
	}

	const dark: TokenMap = new Map(light);
	mergeInto(dark, darkForced);
	return { light, dark, darkAuto, darkForced };
}

/** Lists tokens whose value differs between the two dark blocks. */
export function diffDarkBlocks(themes: ThemeTokens): string[] {
	const names = new Set([
		...themes.darkAuto.keys(),
		...themes.darkForced.keys(),
	]);
	const drift: string[] = [];
	for (const name of names) {
		const auto = themes.darkAuto.get(name);
		const forced = themes.darkForced.get(name);
		if (auto !== forced) {
			drift.push(`${name}: media=${auto ?? "∅"} forced=${forced ?? "∅"}`);
		}
	}
	return drift.sort();
}
