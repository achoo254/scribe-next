/*
 * The brand head tags and the files the web app serves at the site root
 * (web/vite-brand.ts): what the installed web app and browsers load.
 */
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import {
	MANIFEST_PATH,
	brandFiles,
	renderIndexHtml,
} from "../../vite-brand.js";
import { loadBrandConfig } from "../src/index.js";

const brand = loadBrandConfig();
const indexHtml = readFileSync(
	new URL("../../index.html", import.meta.url),
	"utf8",
);

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

	it("serves every file the head links", () => {
		const served = brandFiles(brand);
		const linked = [
			...hrefs(html, "manifest"),
			...hrefs(html, "apple-touch-icon"),
			...hrefs(html, "icon"),
		];
		for (const href of linked) {
			expect(served.has(href.slice(1)), href).toBe(true);
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
	const read = served.get(MANIFEST_PATH);
	const manifest: unknown = JSON.parse(String(read?.()));

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

	it("lists icons the plugin serves, with readable files behind them", () => {
		const icons = manifestIcons(manifest);
		expect(icons.map((icon) => icon.src)).toContain("/favicon.svg");
		for (const { src } of icons) {
			const file = served.get(src.slice(1));
			expect(file, src).toBeDefined();
			expect(file?.().length, src).toBeGreaterThan(0);
		}
		expect(icons.some((icon) => icon.purpose === "maskable")).toBe(true);
	});
});
