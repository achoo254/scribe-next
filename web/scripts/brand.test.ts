/*
 * What Scribe derives from brand.config.json (the rules and the generator
 * are tested in @shared-next/brand): the committed generated files are
 * current, the iPhone app's effective Capacitor config is pinned, the brand
 * colours are the tokens, and web/vite-brand.ts links and serves the
 * manifest and icons.
 */
import { readFileSync } from "node:fs";
import {
	formatGenerated,
	generateManifests,
	iconSourceHashes,
} from "@shared-next/brand/build";
import { describe, expect, it } from "vitest";
import capacitorConfig from "../capacitor.config.js";
import {
	BRAND_GENERATED_DIR,
	BRAND_ROOT,
	MANIFEST_OPTIONS,
	MANIFEST_PATH,
	brandFiles,
	loadScribeBrand,
	renderIndexHtml,
} from "../vite-brand.js";
import { resolveThemes } from "./check-tokens/parse-tokens.js";

const brand = loadScribeBrand();
const generated = (name: string) =>
	readFileSync(new URL(name, BRAND_GENERATED_DIR), "utf8");

describe("brand.config.json", () => {
	it("uses the colours of src/styles/tokens.css", () => {
		const css = readFileSync(
			new URL("src/styles/tokens.css", BRAND_ROOT),
			"utf8",
		);
		const themes = resolveThemes(css);
		expect(brand.colors.primary).toBe(themes.light.get("--c-primary-solid"));
		expect(brand.colors.background).toBe(themes.light.get("--c-bg"));
		expect(brand.colors.backgroundDark).toBe(themes.dark.get("--c-bg"));
	});
});

describe("generated files", () => {
	it("are the manifests of the current brand (run pnpm brand:gen after editing it)", () => {
		const manifests = generateManifests(brand, MANIFEST_OPTIONS);
		expect(Object.keys(manifests).sort()).toEqual([
			"capacitor.json",
			"web.webmanifest.json",
		]);
		for (const [name, content] of Object.entries(manifests)) {
			expect(generated(name), name).toBe(formatGenerated(content));
		}
	});

	it("are icons drawn from the current brand SVGs (run pnpm brand:gen after editing them)", () => {
		const recorded: unknown = JSON.parse(generated("icons/icons-source.json"));
		expect(recorded).toEqual(iconSourceHashes(brand, BRAND_ROOT));
	});
});

describe("Capacitor config", () => {
	it("gives the iPhone app its bundle id and name, and nothing else changes", () => {
		expect(capacitorConfig).toEqual({
			appId: brand.appIds.ios,
			appName: brand.appName,
			webDir: "dist",
			plugins: { CapacitorHttp: { enabled: true } },
		});
	});

	it("keeps the WebView on its own origin, not a tenant's host", () => {
		expect(capacitorConfig).not.toHaveProperty("server");
	});
});

const indexHtml = readFileSync(new URL("index.html", BRAND_ROOT), "utf8");

function hrefs(html: string, rel: string): string[] {
	const pattern = new RegExp(`<link rel="${rel}"[^>]*href="([^"]+)"`, "g");
	return [...html.matchAll(pattern)].map((match) => match[1] ?? "");
}

describe("index.html head", () => {
	const html = renderIndexHtml(indexHtml, brand);

	it("names the app and links the manifest, Apple touch icon and both favicons", () => {
		expect(html).not.toContain("%APP_NAME%");
		expect(html).toContain(`<title>${brand.appName} `);
		expect(hrefs(html, "manifest")).toEqual([`/${MANIFEST_PATH}`]);
		expect(hrefs(html, "apple-touch-icon")).toEqual(["/apple-touch-icon.png"]);
		expect(html).toContain(
			`<meta name="apple-mobile-web-app-title" content="${brand.shortName}" />`,
		);
		// The ICO comes first so browsers with SVG favicons pick the SVG.
		expect(html.indexOf('href="/favicon.ico"')).toBeLessThan(
			html.indexOf('href="/favicon.svg"'),
		);
	});

	it("keeps the single theme-color meta that src/lib/theme.ts updates", () => {
		expect(html.match(/name="theme-color"/g)).toHaveLength(1);
		expect(html).toContain('<meta name="theme-color" content="" />');
	});

	it("serves every file the head links, from a file on disk", () => {
		const served = brandFiles(brand);
		const linked = [
			...hrefs(html, "manifest"),
			...hrefs(html, "apple-touch-icon"),
			...hrefs(html, "icon"),
		];
		for (const href of linked) {
			const read = served.get(href.slice(1));
			expect(read, href).toBeDefined();
			expect(read?.().length, href).toBeGreaterThan(0);
		}
	});

	it("fails when index.html no longer links the SVG favicon", () => {
		expect(() =>
			renderIndexHtml("<head><title>%APP_NAME%</title></head>", brand),
		).toThrow(/favicon\.svg/);
	});
});

interface ManifestIcon {
	readonly src: string;
	readonly purpose: string;
}

function manifestIcons(manifest: unknown): ManifestIcon[] {
	const icons =
		typeof manifest === "object" && manifest !== null
			? Reflect.get(manifest, "icons")
			: undefined;
	if (!Array.isArray(icons)) throw new Error("manifest has no icons array");
	return icons.map((icon: unknown) => {
		const src =
			typeof icon === "object" && icon !== null
				? Reflect.get(icon, "src")
				: undefined;
		const purpose =
			typeof icon === "object" && icon !== null
				? Reflect.get(icon, "purpose")
				: undefined;
		if (typeof src !== "string" || typeof purpose !== "string") {
			throw new Error(
				`manifest icon without src/purpose: ${JSON.stringify(icon)}`,
			);
		}
		return { src, purpose };
	});
}

describe("manifest.webmanifest", () => {
	const served = brandFiles(brand);
	const manifest: unknown = JSON.parse(String(served.get(MANIFEST_PATH)?.()));

	it("is the generated web manifest", () => {
		expect(manifest).toEqual(JSON.parse(generated("web.webmanifest.json")));
	});

	it("opens the app at the site root, inside the BFF sign-in scope", () => {
		expect(manifest).toMatchObject({
			id: "/",
			start_url: "/",
			scope: "/",
			display: "standalone",
			name: brand.appName,
			short_name: brand.shortName,
		});
	});

	it("lists icons the plugin serves, with files on disk behind them", () => {
		const icons = manifestIcons(manifest);
		expect(icons.map((icon) => icon.src)).toContain("/favicon.svg");
		for (const { src } of icons) {
			const read = served.get(src.slice(1));
			expect(read, src).toBeDefined();
			expect(read?.().length, src).toBeGreaterThan(0);
		}
		expect(icons.some((icon) => icon.purpose === "maskable")).toBe(true);
	});
});
