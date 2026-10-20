/*
 * Adapted from chat-next/apps/web/vite-brand.ts @a5be0b4; keep in sync until
 * shared package exists. Scribe changes: no inline splash logo (BrandMark in
 * src/components/brand/wordmark.tsx draws the mark with tokens), no
 * theme-color meta (index.html has the one src/lib/theme.ts keeps on --c-bg;
 * a second one would win over it), and index.html links the SVG favicon
 * itself at /favicon.svg, the URL the site has always used.
 *
 * Brand assets of the web app, derived from brand.config.json and the brand
 * SVGs, so a rebrand is "edit the brand files, pnpm brand:gen, rebuild":
 *
 * - index.html: %APP_NAME% in the title, plus the favicon.ico, Apple touch
 *   icon, manifest and iOS home screen title tags next to the SVG favicon;
 * - the icon files (brand/generated/icons/, made by pnpm brand:gen, and the
 *   brand SVG as /favicon.svg) and manifest.webmanifest at the site root,
 *   served in dev and emitted in builds.
 *
 * No service worker: installing only adds a home screen / app launcher entry
 * that opens start_url "/" in scope "/", so the BFF sign-in redirects stay
 * inside the installed app.
 */
import { readFileSync } from "node:fs";
import type { Plugin } from "vite";
import type { BrandConfig } from "./brand/src/config.js";
import {
	FAVICON_ICO,
	FAVICON_SVG,
	RASTER_ICONS,
	generateManifests,
	loadBrandConfig,
	webIconFiles,
} from "./brand/src/index.js";

/** URL path (no leading slash) of the web app manifest. */
export const MANIFEST_PATH = "manifest.webmanifest";

/** The SVG favicon link index.html carries; the brand tags go right before it. */
const SVG_FAVICON_LINK = `<link rel="icon" type="image/svg+xml" href="/${FAVICON_SVG}" />`;

const HTML_ESCAPES: Record<string, string> = {
	"&": "&amp;",
	"<": "&lt;",
	">": "&gt;",
	'"': "&quot;",
};

function escapeHtml(value: string): string {
	return value.replace(/[&<>"]/g, (char) => HTML_ESCAPES[char] ?? char);
}

const CONTENT_TYPES: Record<string, string> = {
	".svg": "image/svg+xml",
	".png": "image/png",
	".ico": "image/x-icon",
	".webmanifest": "application/manifest+json",
};

function contentType(path: string): string {
	const extension = path.slice(path.lastIndexOf("."));
	return CONTENT_TYPES[extension] ?? "application/octet-stream";
}

/**
 * URL path (no leading slash) to file contents of everything the plugin
 * serves at the site root. Files are read on use, so pnpm brand:gen and an
 * edited brand icon show up without a restart.
 */
export function brandFiles(
	brand: BrandConfig,
): ReadonlyMap<string, () => Buffer | string> {
	const manifest = `${JSON.stringify(generateManifests(brand)["web.webmanifest.json"])}\n`;
	return new Map<string, () => Buffer | string>([
		...webIconFiles(brand).map(
			(icon) => [icon.path, () => readFileSync(icon.file)] as const,
		),
		[MANIFEST_PATH, () => manifest],
	]);
}

/** index.html with the brand name filled in and the brand head tags added. */
export function renderIndexHtml(html: string, brand: BrandConfig): string {
	if (!html.includes(SVG_FAVICON_LINK)) {
		throw new Error(
			`brand: index.html must contain ${SVG_FAVICON_LINK}; the brand tags go before it`,
		);
	}
	const appleTouch = RASTER_ICONS.find((icon) =>
		icon.file.startsWith("apple-touch-icon"),
	);
	if (appleTouch === undefined) {
		throw new Error("brand: no Apple touch icon in RASTER_ICONS");
	}
	// Browsers with SVG favicons take the SVG (linked after the ICO); the
	// others fall back to the ICO.
	const tags = [
		`<link rel="manifest" href="/${MANIFEST_PATH}" />`,
		`<link rel="apple-touch-icon" href="/${appleTouch.file}" sizes="${appleTouch.size}x${appleTouch.size}" />`,
		// The iOS home screen name (iOS reads it before the manifest's short_name).
		`<meta name="apple-mobile-web-app-title" content="${escapeHtml(brand.shortName)}" />`,
		`<link rel="icon" href="/${FAVICON_ICO}" sizes="16x16 32x32 48x48" />`,
	];
	return html
		.replaceAll("%APP_NAME%", escapeHtml(brand.appName))
		.replace(SVG_FAVICON_LINK, [...tags, SVG_FAVICON_LINK].join("\n\t\t"));
}

export function brandAssets(): Plugin {
	const brand = loadBrandConfig();
	const files = brandFiles(brand);

	return {
		name: "scribe-brand-assets",
		transformIndexHtml: {
			order: "pre",
			handler(html) {
				return renderIndexHtml(html, brand);
			},
		},
		configureServer(server) {
			server.middlewares.use((req, res, next) => {
				const path = req.url?.split("?")[0]?.slice(1) ?? "";
				const read = files.get(path);
				if (read === undefined) {
					next();
					return;
				}
				try {
					res.setHeader("Content-Type", contentType(path));
					res.end(read());
				} catch (err) {
					next(err);
				}
			});
		},
		generateBundle() {
			for (const [fileName, read] of files) {
				this.emitFile({ type: "asset", fileName, source: read() });
			}
		},
	};
}
