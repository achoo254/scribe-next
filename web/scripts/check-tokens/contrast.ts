/**
 * Contrast checks for the main text/background pairs of each theme.
 * WCAG 2.x ratio is the gate; APCA Lc (WCAG 3 draft) is reported alongside
 * as an advisory signal.
 */
import { APCAcontrast, sRGBtoY } from "apca-w3";
import {
	CONTRAST_PAIRS,
	type ColorRef,
	type ContrastPair,
} from "./contrast-pairs.js";
import type { TokenMap } from "./parse-tokens.js";

export type Rgb = readonly [number, number, number];

/** Minimum WCAG ratio for normal-size text (SC 1.4.3). */
export const MIN_TEXT_RATIO = 4.5;
/** Minimum WCAG ratio for UI component boundaries and icons (SC 1.4.11). */
export const MIN_UI_RATIO = 3;
/** APCA Lc below this is flagged as a warning for body-size text. */
export const APCA_BODY_TEXT_LC = 60;

export interface ContrastResult {
	theme: string;
	fg: string;
	bg: string;
	kind: ContrastPair["kind"];
	ratio: number;
	apcaLc: number;
	pass: boolean;
}

/** Parses `#rgb`, `#rrggbb` (alpha variants are rejected: unknown backdrop). */
export function parseHexColor(value: string): Rgb {
	const match = /^#([0-9a-f]{3}|[0-9a-f]{6})$/i.exec(value.trim());
	if (!match?.[1]) throw new Error(`Not an opaque hex colour: ${value}`);
	const hex =
		match[1].length === 3
			? [...match[1]].map((digit) => digit + digit).join("")
			: match[1];
	return [
		Number.parseInt(hex.slice(0, 2), 16),
		Number.parseInt(hex.slice(2, 4), 16),
		Number.parseInt(hex.slice(4, 6), 16),
	];
}

/** `color-mix(in srgb, a p%, b)`: linear interpolation of encoded channels. */
export function mixSrgb(a: Rgb, b: Rgb, percent: number): Rgb {
	const t = percent / 100;
	return [
		Math.round(a[0] * t + b[0] * (1 - t)),
		Math.round(a[1] * t + b[1] * (1 - t)),
		Math.round(a[2] * t + b[2] * (1 - t)),
	];
}

function channelToLinear(channel: number): number {
	const c = channel / 255;
	return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
}

export function relativeLuminance(rgb: Rgb): number {
	return (
		0.2126 * channelToLinear(rgb[0]) +
		0.7152 * channelToLinear(rgb[1]) +
		0.0722 * channelToLinear(rgb[2])
	);
}

export function wcagRatio(fg: Rgb, bg: Rgb): number {
	const a = relativeLuminance(fg);
	const b = relativeLuminance(bg);
	return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
}

export function apcaLc(fg: Rgb, bg: Rgb): number {
	return Number(APCAcontrast(sRGBtoY([...fg]), sRGBtoY([...bg])));
}

function resolveToken(tokens: TokenMap, name: string, depth = 0): Rgb {
	const value = tokens.get(name);
	if (value === undefined) throw new Error(`Unknown token ${name}`);
	const alias = /^var\((--[\w-]+)\)$/.exec(value.trim());
	if (alias?.[1]) {
		if (depth > 8) throw new Error(`Token alias loop at ${name}`);
		return resolveToken(tokens, alias[1], depth + 1);
	}
	return parseHexColor(value);
}

function resolveRef(tokens: TokenMap, ref: ColorRef): Rgb {
	if (typeof ref === "string") return resolveToken(tokens, ref);
	return mixSrgb(
		resolveToken(tokens, ref.mix),
		resolveToken(tokens, ref.into),
		ref.percent,
	);
}

function labelOf(ref: ColorRef): string {
	return typeof ref === "string" ? ref : ref.label;
}

export function evaluatePairs(
	theme: string,
	tokens: TokenMap,
	pairs: readonly ContrastPair[] = CONTRAST_PAIRS,
): ContrastResult[] {
	return pairs.map((pair) => {
		const fg = resolveRef(tokens, pair.fg);
		const bg = resolveRef(tokens, pair.bg);
		const ratio = wcagRatio(fg, bg);
		const min = pair.kind === "text" ? MIN_TEXT_RATIO : MIN_UI_RATIO;
		return {
			theme,
			fg: labelOf(pair.fg),
			bg: labelOf(pair.bg),
			kind: pair.kind,
			ratio,
			apcaLc: apcaLc(fg, bg),
			pass: ratio >= min,
		};
	});
}
