// Minimal typings for the subset of apca-w3 (ships without types) used by
// the token contrast check.
declare module "apca-w3" {
	/** Screen luminance Y from an sRGB 8-bit triplet (or a hex number/string). */
	export function sRGBtoY(rgb: readonly number[] | number | string): number;
	/** APCA lightness contrast Lc; negative means light text on dark bg. */
	export function APCAcontrast(
		textY: number,
		backgroundY: number,
		places?: number,
	): number | string;
}
