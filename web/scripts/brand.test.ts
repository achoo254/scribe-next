/*
 * What Scribe derives from brand.config.json (the rules and the generator
 * are tested in @shared-next/brand): the committed generated files are
 * current, the iPhone app's effective Capacitor config is pinned, the
 * Android app's id and launcher resources follow the brand, the brand
 * colours are the tokens, and web/vite-brand.ts links and serves the
 * manifest and icons.
 */
import { existsSync, readFileSync, readdirSync } from "node:fs";
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
	OG_IMAGE,
	brandFiles,
	loadScribeBrand,
	renderIndexHtml,
} from "../vite-brand.js";
import {
	ANDROID_RES_DIR,
	LEGACY_DENSITIES,
	PLAY_DIR,
	VECTOR_ONLY_DRAWABLES,
	adaptiveLayerSize,
	androidResourceFiles,
	isPictureIcon,
	readMonochromeIcon,
} from "./brand-android.js";
import {
	IOS_SPLASH_DIR,
	IOS_SPLASH_FILES,
	iosSplashSvg,
} from "./brand-ios-splash.js";
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

/** Width and height of a PNG (IHDR) or a baseline/progressive JPEG (SOF0/SOF2), plus JPEG components. */
function imageSize(bytes: Buffer): {
	width: number;
	height: number;
	components?: number;
} {
	if (bytes.subarray(1, 4).toString("ascii") === "PNG") {
		return { width: bytes.readUInt32BE(16), height: bytes.readUInt32BE(20) };
	}
	let offset = 2;
	while (offset + 9 < bytes.length) {
		const marker = bytes[offset + 1];
		const length = bytes.readUInt16BE(offset + 2);
		if (marker === 0xc0 || marker === 0xc2) {
			return {
				height: bytes.readUInt16BE(offset + 5),
				width: bytes.readUInt16BE(offset + 7),
				components: bytes[offset + 9],
			};
		}
		offset += 2 + length;
	}
	throw new Error("not a PNG or JPEG");
}

const iosSplashDir = new URL(IOS_SPLASH_DIR, BRAND_ROOT);

describe("iOS splash", () => {
	it("centres the brand icon on the background colour", () => {
		const svg = iosSplashSvg('<svg id="icon"/>', "#f9f9fb");
		expect(svg).toContain('fill="#f9f9fb"');
		expect(svg).toContain(
			`href="data:image/svg+xml;base64,${Buffer.from('<svg id="icon"/>').toString("base64")}"`,
		);
		expect(svg).toContain('x="1066" y="1066" width="601" height="601"');
	});

	it.skipIf(!existsSync(iosSplashDir))(
		"is one 2732 px picture in all three files (run pnpm brand:gen)",
		() => {
			const files = IOS_SPLASH_FILES.map((file) =>
				readFileSync(new URL(file, iosSplashDir)),
			);
			for (const file of files) {
				expect(imageSize(file)).toEqual({ width: 2732, height: 2732 });
				expect(file.equals(files[0] ?? Buffer.alloc(0))).toBe(true);
			}
		},
	);
});

const androidDir = new URL("android/", BRAND_ROOT);
const hasAndroid = existsSync(androidDir);

describe("Android app", () => {
	it("uses one application id with iOS (Capacitor has a single appId)", () => {
		expect(brand.appIds.android).toBe(brand.appIds.ios);
	});

	it.runIf(hasAndroid)("builds with the brand's application id", () => {
		const gradle = readFileSync(
			new URL("app/build.gradle", androidDir),
			"utf8",
		);
		expect(gradle).toContain(`applicationId "${brand.appIds.android}"`);
		expect(gradle).toContain(`namespace = "${brand.appIds.android}"`);
	});

	it.runIf(hasAndroid)(
		"has the launcher icon and splash colours of the current brand (run pnpm brand:gen)",
		() => {
			const maskable = readFileSync(
				new URL(brand.iconMaskable, BRAND_ROOT),
				"utf8",
			);
			const monochrome = readMonochromeIcon(BRAND_ROOT);
			const resDir = new URL(ANDROID_RES_DIR, BRAND_ROOT);
			for (const [path, content] of Object.entries(
				androidResourceFiles(maskable, brand.colors, monochrome),
			)) {
				expect(readFileSync(new URL(path, resDir), "utf8"), path).toBe(content);
			}
			for (const { dir, size } of LEGACY_DENSITIES) {
				for (const name of ["ic_launcher.png", "ic_launcher_round.png"]) {
					const png = readFileSync(new URL(`${dir}/${name}`, resDir));
					expect(imageSize(png), `${dir}/${name}`).toEqual({
						width: size,
						height: size,
					});
				}
			}
			// A picture icon has PNG foreground (and themed) layers instead of the vector ones.
			const picture = isPictureIcon(maskable);
			const pngLayers = [
				["ic_launcher_foreground.png", picture],
				["ic_launcher_monochrome.png", picture && monochrome !== null],
			] as const;
			for (const { dir, size } of LEGACY_DENSITIES) {
				for (const [name, present] of pngLayers) {
					const file = new URL(`${dir}/${name}`, resDir);
					expect(existsSync(file), `${dir}/${name}`).toBe(present);
					if (present) {
						const layer = adaptiveLayerSize(size);
						expect(imageSize(readFileSync(file))).toEqual({
							width: layer,
							height: layer,
						});
					}
				}
			}
			for (const path of VECTOR_ONLY_DRAWABLES) {
				expect(existsSync(new URL(path, resDir)), path).toBe(!picture);
			}
		},
	);

	it.runIf(hasAndroid)(
		"resolves every drawable and mipmap the theme and manifest name (brand:gen removes some)",
		() => {
			const resDir = new URL(ANDROID_RES_DIR, BRAND_ROOT);
			const dirs = readdirSync(resDir);
			const values = dirs
				.filter((dir) => dir === "values" || dir.startsWith("values-"))
				.flatMap((dir) =>
					readdirSync(new URL(`${dir}/`, resDir)).map((file) =>
						readFileSync(new URL(`${dir}/${file}`, resDir), "utf8"),
					),
				)
				.join("\n");
			const exists = (type: string, name: string): boolean =>
				values.includes(`<item name="${name}" type="${type}">`) ||
				dirs
					.filter((dir) => dir === type || dir.startsWith(`${type}-`))
					.some((dir) =>
						readdirSync(new URL(`${dir}/`, resDir)).some(
							(file) => file.replace(/\..*$/, "") === name,
						),
					);
			const referrers = [
				`${ANDROID_RES_DIR}values/styles.xml`,
				"android/app/src/main/AndroidManifest.xml",
			];
			for (const path of referrers) {
				const xml = readFileSync(new URL(path, BRAND_ROOT), "utf8");
				for (const [ref, type, name] of xml.matchAll(
					/@(drawable|mipmap)\/(\w+)/g,
				)) {
					expect(exists(type ?? "", name ?? ""), `${path}: ${ref}`).toBe(true);
				}
			}
		},
	);

	it("gives a picture icon a themed layer only with its silhouette", () => {
		const maskable = readFileSync(
			new URL(brand.iconMaskable, BRAND_ROOT),
			"utf8",
		);
		const monochrome = readMonochromeIcon(BRAND_ROOT);
		expect(monochrome).not.toBeNull();
		const adaptive = "mipmap-anydpi-v26/ic_launcher.xml";
		expect(
			androidResourceFiles(maskable, brand.colors, monochrome)[adaptive],
		).toContain(
			'<monochrome android:drawable="@mipmap/ic_launcher_monochrome"/>',
		);
		if (isPictureIcon(maskable)) {
			expect(
				androidResourceFiles(maskable, brand.colors, null)[adaptive],
			).not.toContain("<monochrome");
		}
	});

	it("has the Play listing icon and feature graphic", () => {
		const icon = readFileSync(
			new URL(`${PLAY_DIR}icon-512.png`, BRAND_GENERATED_DIR),
		);
		expect(imageSize(icon)).toEqual({ width: 512, height: 512 });
		// JPEG: no alpha channel, as Play requires for the feature graphic.
		const feature = readFileSync(
			new URL(`${PLAY_DIR}feature-graphic.jpg`, BRAND_GENERATED_DIR),
		);
		expect(imageSize(feature)).toEqual({
			width: 1024,
			height: 500,
			components: 3,
		});
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

	it("describes the app once, with brand.description", () => {
		expect(brand.description.trim()).not.toBe("");
		expect(html.match(/name="description"/g)).toHaveLength(1);
		expect(html).toContain(
			`<meta name="description" content="${brand.description}" />`,
		);
	});

	it("gives link previews the app name, description and the 1200×630 image", () => {
		const image = `https://scribe.hoandat.com/${OG_IMAGE.path}`;
		expect(html).toContain(
			`<meta property="og:title" content="${brand.appName}" />`,
		);
		expect(html).toContain(
			`<meta property="og:description" content="${brand.description}" />`,
		);
		expect(html).toContain(`<meta property="og:image" content="${image}" />`);
		expect(html).toContain(`<meta name="twitter:image" content="${image}" />`);
		expect(html).toContain(
			'<meta name="twitter:card" content="summary_large_image" />',
		);
		// JPEG start-of-frame: height then width, big-endian, after the marker.
		const jpeg = readFileSync(new URL(`public/${OG_IMAGE.path}`, BRAND_ROOT));
		const sof = jpeg.findIndex(
			(byte, i) =>
				byte === 0xff && (jpeg[i + 1] === 0xc0 || jpeg[i + 1] === 0xc2),
		);
		expect(sof).toBeGreaterThan(0);
		expect(jpeg.readUInt16BE(sof + 7)).toBe(OG_IMAGE.width);
		expect(jpeg.readUInt16BE(sof + 5)).toBe(OG_IMAGE.height);
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
