/**
 * Catalogue of text/background pairs checked in every theme. Pairs mirror
 * how the primitives combine tokens (see tokens.css header for the rules).
 */

/** A colour reference: a token, or a token mixed into another (srgb). */
export type ColorRef =
	| string
	| { mix: string; percent: number; into: string; label: string };

export interface ContrastPair {
	fg: ColorRef;
	bg: ColorRef;
	kind: "text" | "ui";
}

/** Background tint used by soft labels: `color-mix(in srgb, X 8%, surface)`. */
function tint(token: string, percent = 8): ColorRef {
	return {
		mix: token,
		percent,
		into: "--c-surface",
		label: `${token}@${percent}%`,
	};
}

/** Soft-label foreground on its own hue tint (badges, alerts, chips). */
const SOFT_LABELS: ReadonlyArray<readonly [string, string]> = [
	["--c-text-muted", "--c-text-muted"],
	["--c-primary", "--c-primary"],
	["--c-accent", "--c-accent"],
	["--c-success-text", "--c-success"],
	["--c-warning-text", "--c-warning"],
	["--c-error-text", "--c-error"],
	["--c-info-text", "--c-info"],
	["--c-viz-green-text", "--c-viz-green"],
	["--c-viz-amber-text", "--c-viz-amber"],
	["--c-viz-blue-text", "--c-viz-blue"],
	["--c-viz-cyan-text", "--c-viz-cyan"],
	["--c-viz-violet-text", "--c-viz-violet"],
	["--c-viz-red-text", "--c-viz-red"],
	["--c-viz-grey-text", "--c-viz-grey"],
];

/** Neutral and brand text: allowed on every page/card background. */
const TEXT_ON_ANY_BACKGROUND = [
	"--c-text",
	"--c-text-muted",
	"--c-primary",
	"--c-accent",
];

/**
 * Semantic step-11 text is only allowed on the solid --c-surface (see the
 * tokens.css header): light green-11 / amber-11 fall to ~4.4:1 on slate-2.
 */
const SEMANTIC_TEXT_ON_SURFACE = [
	"--c-info",
	"--c-success",
	"--c-warning",
	"--c-error",
];

export const CONTRAST_PAIRS: readonly ContrastPair[] = [
	...TEXT_ON_ANY_BACKGROUND.flatMap((fg): ContrastPair[] => [
		{ fg, bg: "--c-surface", kind: "text" },
		{ fg, bg: "--c-bg", kind: "text" },
		{ fg, bg: "--c-surface-subtle", kind: "text" },
	]),
	...SEMANTIC_TEXT_ON_SURFACE.map(
		(fg): ContrastPair => ({ fg, bg: "--c-surface", kind: "text" }),
	),
	{ fg: "--c-on-primary", bg: "--c-primary-solid", kind: "text" },
	{ fg: "--c-on-primary", bg: "--c-primary-solid-hover", kind: "text" },
	{ fg: "--c-on-tone", bg: "--c-error", kind: "text" },
	{ fg: "--c-on-tone", bg: "--c-success", kind: "text" },
	{ fg: "--c-on-tone", bg: "--c-warning", kind: "text" },
	{ fg: "--c-on-tone", bg: "--c-accent", kind: "text" },
	// Tooltip: inverted surface.
	{ fg: "--c-surface", bg: "--c-text", kind: "text" },
	// Alert body text sits on a 8% tone tint.
	{ fg: "--c-text", bg: tint("--c-warning"), kind: "text" },
	{ fg: "--c-text", bg: tint("--c-error"), kind: "text" },
	...SOFT_LABELS.map(
		([fg, hue]): ContrastPair => ({ fg, bg: tint(hue), kind: "text" }),
	),
	{ fg: "--c-icon-muted", bg: "--c-surface", kind: "ui" },
	{ fg: "--c-border-strong", bg: "--c-surface", kind: "ui" },
];
