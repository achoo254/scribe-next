/*
 * Copied from chat-next/packages/brand @a5be0b4; keep in sync until shared
 * package exists. Scribe change: Chromium comes from @playwright/test, the
 * e2e runner already installed here, instead of a separate playwright dep.
 *
 * Draws the brand SVGs into PNG files with Playwright's Chromium, so no
 * image library is needed. Used by `pnpm brand:gen` only; nothing at
 * runtime imports it.
 */
import { readFileSync } from "node:fs";
import { chromium } from "@playwright/test";
import { WEB_ROOT } from "./config.js";

export interface RasterRequest {
	/** web/-relative path of the SVG. */
	readonly svg: string;
	readonly size: number;
}

/** Returns one PNG per request, in order, with a transparent background. */
export async function rasterize(
	requests: readonly RasterRequest[],
): Promise<Uint8Array[]> {
	const browser = await chromium.launch();
	try {
		const page = await browser.newPage({ deviceScaleFactor: 1 });
		const pngs: Uint8Array[] = [];
		for (const { svg, size } of requests) {
			const data = readFileSync(new URL(svg, WEB_ROOT)).toString("base64");
			await page.setViewportSize({ width: size, height: size });
			await page.setContent(
				`<html><body style="margin:0;background:transparent"><img id="i" width="${size}" height="${size}" style="display:block" src="data:image/svg+xml;base64,${data}"></body></html>`,
			);
			await page.locator("#i").waitFor();
			await page.waitForFunction("document.getElementById('i').complete");
			pngs.push(
				await page.screenshot({
					omitBackground: true,
					clip: { x: 0, y: 0, width: size, height: size },
				}),
			);
		}
		return pngs;
	} finally {
		await browser.close();
	}
}
