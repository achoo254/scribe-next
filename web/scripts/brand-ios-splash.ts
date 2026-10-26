/**
 * The iOS launch screen picture (ios/App/App/Assets.xcassets/Splash.imageset): the brand icon centred on
 * colors.background, 2732 px square. Xcode's set lists the same picture three times (1x, 2x, 3x), so all
 * three files are written with the same pixels. Written only when web/ios/ exists.
 */
import { existsSync, writeFileSync } from "node:fs";
import type { IconRenderer } from "@shared-next/brand/build";

/** Where the splash image set lives, relative to web/. */
export const IOS_SPLASH_DIR = "ios/App/App/Assets.xcassets/Splash.imageset/";
export const IOS_SPLASH_FILES = [
	"splash-2732x2732.png",
	"splash-2732x2732-1.png",
	"splash-2732x2732-2.png",
] as const;
/** iOS fills the screen with the square ("aspect fill"). */
const SPLASH = 2732;
/** Icon side as a share of the square, the size the launch screen has always shown it at. */
const ICON_SHARE = 0.22;

/** The splash as SVG: the icon (as an embedded SVG image) centred on the background colour. */
export function iosSplashSvg(iconSvg: string, background: string): string {
	const side = Math.round(SPLASH * ICON_SHARE);
	const offset = Math.round((SPLASH - side) / 2);
	const icon = Buffer.from(iconSvg).toString("base64");
	return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${SPLASH} ${SPLASH}"><rect width="${SPLASH}" height="${SPLASH}" fill="${background}"/><image x="${offset}" y="${offset}" width="${side}" height="${side}" href="data:image/svg+xml;base64,${icon}"/></svg>`;
}

export interface WriteIosSplashOptions {
	/** web/ */
	readonly root: URL;
	readonly iconSvg: string;
	readonly background: string;
	readonly renderer: IconRenderer;
}

/** Writes the three splash files when web/ios/ exists; returns the paths written. */
export async function writeIosSplash(
	options: WriteIosSplashOptions,
): Promise<string[]> {
	const dir = new URL(IOS_SPLASH_DIR, options.root);
	if (!existsSync(dir)) return [];
	const [png] = await options.renderer([
		{
			path: "iOS splash",
			svg: Buffer.from(iosSplashSvg(options.iconSvg, options.background)),
			size: SPLASH,
		},
	]);
	if (png === undefined)
		throw new Error("brand ios: renderer returned no splash");
	for (const file of IOS_SPLASH_FILES) writeFileSync(new URL(file, dir), png);
	return IOS_SPLASH_FILES.map((file) => `${IOS_SPLASH_DIR}${file}`);
}
