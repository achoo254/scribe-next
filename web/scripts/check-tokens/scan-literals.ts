/**
 * Finds design-token bypasses in web sources: colour literals and raw px
 * spacing. `src/styles/tokens.css` is the only file allowed to
 * hold those values, so it is never passed to this scanner.
 */

export type LiteralRule = "hex-color" | "color-function" | "spacing-px";

/** Which spacing syntaxes apply: CSS declarations or JS style/class strings. */
export type SourceKind = "stylesheet" | "script";

export interface LiteralViolation {
	file: string;
	line: number;
	rule: LiteralRule;
	match: string;
}

/**
 * A line containing this marker followed by a non-empty reason is skipped.
 * The reason is mandatory so every exception documents itself.
 */
export const IGNORE_MARKER = "check-tokens-ignore:";

/**
 * Raw px values tolerated inside spacing declarations, with the reason.
 * Everything else must come from a `--space-*` token or the Tailwind
 * spacing scale (which multiplies `--space-1`).
 */
export const ALLOWED_SPACING_PX: ReadonlyMap<string, string> = new Map([
	["0px", "zero is unit-less in intent; some tools emit it with a unit"],
	["1px", "hairline offsets that align content with a 1px border"],
	["2px", "focus-ring compensation matching the 2px outline offset"],
]);

const HEX_COLOR =
	/(?<![&\w])#(?:[0-9a-fA-F]{8}|[0-9a-fA-F]{6}|[0-9a-fA-F]{3,4})(?![\w-])/g;

const COLOR_FUNCTION = /\b(?:rgba?|hsla?|hwb|oklch|oklab)\(/gi;

const PX_VALUE = /-?\d*\.?\d+px\b/g;

/** CSS declaration whose property sets spacing: `padding-top: 12px`. */
const CSS_SPACING_DECL =
	/(?:^|[;{\s])((?:padding|margin|scroll-padding|scroll-margin|inset)(?:-[a-z-]+)?|gap|row-gap|column-gap|top|right|bottom|left)\s*:\s*([^;}]*)/g;

/** React style object entry with a string value: `paddingTop: "12px"`. */
const STYLE_SPACING_ENTRY =
	/\b((?:padding|margin|scrollPadding|scrollMargin|inset)\w*|gap|rowGap|columnGap|top|right|bottom|left)\s*:\s*(["'`])([^"'`]*)\2/g;

/** Tailwind arbitrary spacing class: `p-[12px]`, `md:-mt-[3px]`, `gap-x-[6px]`. */
const TAILWIND_SPACING_ARBITRARY =
	/(?<=^|[\s"'`:!])-?(?:p[xytrblse]?|m[xytrblse]?|gap(?:-[xy])?|inset(?:-[xy])?|top|right|bottom|left|start|end|space-[xy]|scroll-[mp][xytrblse]?)-\[([^\]]+)\]/g;

function disallowedPx(value: string): string[] {
	const found = value.match(PX_VALUE) ?? [];
	return found.filter((px) => !ALLOWED_SPACING_PX.has(px.replace(/^-/, "")));
}

function hasIgnoreMarker(line: string): boolean {
	const index = line.indexOf(IGNORE_MARKER);
	if (index === -1) return false;
	return line.slice(index + IGNORE_MARKER.length).trim().length > 0;
}

export function sourceKindOf(file: string): SourceKind {
	return /\.(?:tsx?|jsx?|mjs|cjs)$/.test(file) ? "script" : "stylesheet";
}

function scanLine(
	line: string,
	kind: SourceKind,
): Array<{ rule: LiteralRule; match: string }> {
	const hits: Array<{ rule: LiteralRule; match: string }> = [];

	for (const m of line.matchAll(HEX_COLOR)) {
		hits.push({ rule: "hex-color", match: m[0] });
	}
	for (const m of line.matchAll(COLOR_FUNCTION)) {
		hits.push({ rule: "color-function", match: m[0] });
	}
	if (kind === "stylesheet") {
		for (const m of line.matchAll(CSS_SPACING_DECL)) {
			for (const px of disallowedPx(m[2] ?? "")) {
				hits.push({ rule: "spacing-px", match: `${m[1]}: ${px}` });
			}
		}
	} else {
		for (const m of line.matchAll(STYLE_SPACING_ENTRY)) {
			for (const px of disallowedPx(m[3] ?? "")) {
				hits.push({ rule: "spacing-px", match: `${m[1]}: ${px}` });
			}
		}
	}
	// Tailwind classes appear in scripts and in stylesheet `@apply` rules.
	for (const m of line.matchAll(TAILWIND_SPACING_ARBITRARY)) {
		if (disallowedPx(m[1] ?? "").length > 0) {
			hits.push({ rule: "spacing-px", match: m[0] });
		}
	}
	return hits;
}

/** Scans one file's text and reports every violation with a 1-based line. */
export function findLiteralViolations(
	source: string,
	file: string,
): LiteralViolation[] {
	const kind = sourceKindOf(file);
	const violations: LiteralViolation[] = [];
	const lines = source.split(/\r?\n/);
	lines.forEach((line, index) => {
		if (hasIgnoreMarker(line)) return;
		for (const hit of scanLine(line, kind)) {
			violations.push({ file, line: index + 1, ...hit });
		}
	});
	return violations;
}
