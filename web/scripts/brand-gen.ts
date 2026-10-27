/**
 * `pnpm run brand:gen`: validates brand.config.json against the
 * @achoo254/brand schema and writes what the platforms derive from it to
 * brand/generated/: the web manifest, capacitor.json (spread by
 * capacitor.config.ts) and the PNG/ICO icons drawn from the brand SVGs with
 * the Chromium of @playwright/test, plus the Android launcher icons, splash
 * colours and Play listing art (scripts/brand-android.ts), and the iOS
 * launch screen picture (scripts/brand-ios-splash.ts). Run from web/ after
 * editing the brand. The iOS AppIcon (ios/App/App/Assets.xcassets) is not
 * generated: it ships as is.
 */
import { readFileSync } from "node:fs";
import { chromiumRenderer, writeBrandFiles } from "@achoo254/brand/build";
import { chromium } from "@playwright/test";
import {
	BRAND_GENERATED_DIR,
	BRAND_ROOT,
	MANIFEST_OPTIONS,
	loadScribeBrand,
} from "../vite-brand.js";
import { readMonochromeIcon, writeAndroidBrandFiles } from "./brand-android.js";
import { writeIosSplash } from "./brand-ios-splash.js";

async function main(): Promise<void> {
	const brand = loadScribeBrand();
	const renderer = chromiumRenderer(chromium);
	const written = await writeBrandFiles({
		brand,
		root: BRAND_ROOT,
		generatedDir: BRAND_GENERATED_DIR,
		manifests: MANIFEST_OPTIONS,
		indent: 2,
		renderer,
	});
	console.log(
		`Wrote ${written.length} files to ${BRAND_GENERATED_DIR.pathname}: ${written.join(", ")}`,
	);
	const android = await writeAndroidBrandFiles({
		root: BRAND_ROOT,
		generatedDir: BRAND_GENERATED_DIR,
		iconSvg: readFileSync(new URL(brand.icon, BRAND_ROOT), "utf8"),
		maskableSvg: readFileSync(new URL(brand.iconMaskable, BRAND_ROOT), "utf8"),
		monochromeSvg: readMonochromeIcon(BRAND_ROOT),
		appName: brand.appName,
		colors: brand.colors,
		renderer,
		chromium,
	});
	console.log(
		`Wrote ${android.length} Android/Play files: ${android.join(", ")}`,
	);
	const ios = await writeIosSplash({
		root: BRAND_ROOT,
		iconSvg: readFileSync(new URL(brand.icon, BRAND_ROOT), "utf8"),
		background: brand.colors.background,
		renderer,
	});
	console.log(`Wrote ${ios.length} iOS splash files: ${ios.join(", ")}`);
}

main().catch((error: unknown) => {
	console.error("brand:gen failed:", error);
	process.exitCode = 1;
});
