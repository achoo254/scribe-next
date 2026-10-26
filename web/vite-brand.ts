/*
 * Brand assets of the web app, derived from brand.config.json and the brand
 * SVGs through @shared-next/brand, so a rebrand is "edit the brand files,
 * pnpm brand:gen, rebuild":
 *
 * - index.html: %APP_NAME% in the title, plus the description, favicon.ico,
 *   Apple touch icon, manifest and iOS home screen title tags next to the SVG
 *   favicon;
 * - the icon files (brand/generated/icons/, made by pnpm brand:gen, and the
 *   brand SVG as /favicon.svg) and manifest.webmanifest at the site root,
 *   served in dev and emitted in builds.
 *
 * Scribe specifics: no inline splash logo (BrandMark in
 * src/components/brand/wordmark.tsx shows brand/scribe-mark-128.webp), no
 * theme-color meta (index.html has the one src/lib/theme.ts keeps on --c-bg;
 * a second one would win over it), and index.html links the SVG favicon
 * itself at /favicon.svg, the URL the site has always used.
 *
 * No service worker: installing only adds a home screen / app launcher entry
 * that opens start_url "/" in scope "/", so the BFF sign-in redirects stay
 * inside the installed app.
 *
 * The options below are also what scripts/brand-gen.ts writes
 * brand/generated/ with, so the served manifest and the generated one match.
 */
import { readFileSync } from "node:fs";
import {
	type BrandConfigWith,
	FAVICON_ICO,
	type GenerateManifestsOptions,
	RASTER_ICONS,
	generateManifests,
	loadBrandConfig,
	svgManifestIcons,
	webIconFiles,
} from "@shared-next/brand/build";
import type { Plugin } from "vite";

/** web/, where brand.config.json and the icon paths inside it are rooted. */
export const BRAND_ROOT = new URL("./", import.meta.url);

/** Where pnpm brand:gen writes the manifests and icons. */
export const BRAND_GENERATED_DIR = new URL("brand/generated/", BRAND_ROOT);

/** URL path of the SVG favicon (a copy of the brand icon). */
const FAVICON_SVG = "favicon.svg";

/** Optional schema fields Scribe needs (src/styles/tokens.css colours). */
const BRAND_REQUIRE = ["colors.primary", "colors.backgroundDark"] as const;

export type ScribeBrand = BrandConfigWith<(typeof BRAND_REQUIRE)[number]>;

/** Reads web/brand.config.json and validates it against the shared schema. */
export function loadScribeBrand(): ScribeBrand {
	return loadBrandConfig(BRAND_ROOT, { require: BRAND_REQUIRE });
}

/** The platform manifests Scribe ships (brand/generated/). */
export const MANIFEST_OPTIONS: GenerateManifestsOptions = {
	web: {
		// A stable identity: browsers treat the installed app as the same one across start_url changes.
		id: "/",
		lang: "vi",
		dir: "ltr",
		// The <meta name="theme-color"> that src/lib/theme.ts keeps on --c-bg.
		themeColor: "background",
		categories: ["productivity"],
		preferRelatedApplications: false,
		icons: svgManifestIcons(FAVICON_SVG),
	},
	// capacitor.config.ts spreads only appId and appName. No server.hostname:
	// the WebView keeps capacitor://localhost (lib/api.ts adds the backend origin).
	capacitor: { urlScheme: false },
};

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
	brand: ScribeBrand,
): ReadonlyMap<string, () => Buffer | string> {
	const manifest = `${JSON.stringify(generateManifests(brand, MANIFEST_OPTIONS)["web.webmanifest.json"])}\n`;
	const icons = webIconFiles(brand, {
		root: BRAND_ROOT,
		generatedDir: BRAND_GENERATED_DIR,
		faviconSvg: FAVICON_SVG,
	});
	return new Map<string, () => Buffer | string>([
		...icons.map((icon) => [icon.path, () => readFileSync(icon.file)] as const),
		[MANIFEST_PATH, () => manifest],
	]);
}

/** index.html with the brand name filled in and the brand head tags added. */
export function renderIndexHtml(html: string, brand: ScribeBrand): string {
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
		// Search results and link previews (brand.description is Vietnamese, like <html lang="vi">).
		`<meta name="description" content="${escapeHtml(brand.description)}" />`,
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
	const brand = loadScribeBrand();
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
