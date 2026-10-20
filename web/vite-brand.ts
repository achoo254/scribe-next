/*
 * Adapted from chat-next/apps/web/vite-brand.ts @a5be0b4; keep in sync until
 * shared package exists. Scribe changes: no inline splash logo (BrandMark in
 * src/components/brand/wordmark.tsx draws the mark with tokens), no
 * theme-color meta (src/lib/theme.ts keeps it on --c-bg), and only the SVG
 * favicon is served. The web manifest, Apple touch icon and PNG/ICO icons
 * that pnpm brand:gen writes to brand/generated/ are not linked, served or
 * emitted: making the site installable is a separate product decision.
 *
 * Brand assets of the web app, derived from brand.config.json and the brand
 * icon, so a rebrand is "edit the brand files, rebuild":
 *
 * - index.html: %APP_NAME% in the title (index.html links /favicon.svg itself);
 * - /favicon.svg: the brand icon (brand.config.json "icon"), served in dev
 *   and emitted in builds.
 */
import { readFileSync } from "node:fs";
import type { Plugin } from "vite";
import { WEB_ROOT, loadBrandConfig } from "./brand/src/config.js";

/** URL path (no leading slash) index.html links as the favicon. */
const FAVICON_PATH = "favicon.svg";

const HTML_ESCAPES: Record<string, string> = {
	"&": "&amp;",
	"<": "&lt;",
	">": "&gt;",
	'"': "&quot;",
};

function escapeHtml(value: string): string {
	return value.replace(/[&<>"]/g, (char) => HTML_ESCAPES[char] ?? char);
}

export function brandAssets(): Plugin {
	const brand = loadBrandConfig();
	const iconFile = new URL(brand.icon, WEB_ROOT);
	/** Read on use, so an edited brand icon shows up without a restart. */
	const readFavicon = (): Buffer => readFileSync(iconFile);

	return {
		name: "scribe-brand-assets",
		transformIndexHtml: {
			order: "pre",
			handler(html) {
				return html.replaceAll("%APP_NAME%", escapeHtml(brand.appName));
			},
		},
		configureServer(server) {
			server.middlewares.use((req, res, next) => {
				const path = req.url?.split("?")[0]?.slice(1) ?? "";
				if (path !== FAVICON_PATH) {
					next();
					return;
				}
				try {
					res.setHeader("Content-Type", "image/svg+xml");
					res.end(readFavicon());
				} catch (err) {
					next(err);
				}
			});
		},
		generateBundle() {
			this.emitFile({
				type: "asset",
				fileName: FAVICON_PATH,
				source: readFavicon(),
			});
		},
	};
}
