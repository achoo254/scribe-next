/**
 * `pnpm run brand:gen`: validates brand.config.json against the
 * @shared-next/brand schema and writes what the platforms derive from it to
 * brand/generated/: the web manifest, capacitor.json (spread by
 * capacitor.config.ts) and the PNG/ICO icons drawn from the brand SVGs with
 * the Chromium of @playwright/test. Run from web/ after editing the brand.
 * The iOS AppIcon (ios/App/App/Assets.xcassets) is not generated: it ships
 * as is.
 */
import { chromium } from "@playwright/test";
import { chromiumRenderer, writeBrandFiles } from "@shared-next/brand/build";
import {
	BRAND_GENERATED_DIR,
	BRAND_ROOT,
	MANIFEST_OPTIONS,
	loadScribeBrand,
} from "../vite-brand.js";

async function main(): Promise<void> {
	const written = await writeBrandFiles({
		brand: loadScribeBrand(),
		root: BRAND_ROOT,
		generatedDir: BRAND_GENERATED_DIR,
		manifests: MANIFEST_OPTIONS,
		indent: 2,
		renderer: chromiumRenderer(chromium),
	});
	console.log(
		`Wrote ${written.length} files to ${BRAND_GENERATED_DIR.pathname}: ${written.join(", ")}`,
	);
}

main().catch((error: unknown) => {
	console.error("brand:gen failed:", error);
	process.exitCode = 1;
});
