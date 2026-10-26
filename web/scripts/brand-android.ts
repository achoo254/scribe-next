/**
 * Android and Google Play assets drawn from the brand (part of `pnpm run brand:gen`):
 *
 * - the adaptive launcher icon as vector drawables (background, foreground, monochrome for themed icons
 *   on Android 13+), converted from brand.config.json "iconMaskable": the SVG's full-canvas rects become
 *   the background layer, its <g> artwork the foreground, scaled so the maskable safe circle (80 %)
 *   lands in the launcher's 66 dp safe zone. A picture icon (artwork embedded as an <image>) cannot be
 *   converted: its foreground is then the whole maskable picture as PNGs in every mipmap density (same
 *   safe-circle scaling) over a vector background in colors.primary, and its monochrome layer the
 *   white silhouette of brand/icon-monochrome.svg (framed like the maskable icon) drawn the same way;
 *   without that file there is no monochrome layer (Android 13+ shows the icon in its own colours);
 * - legacy PNG launcher icons (Android 7.x, below adaptive icons) in every mipmap density;
 * - the splash (Android 12+ SplashScreen: the foreground icon on colors.background, dark theme
 *   colors.backgroundDark): its colours, and the drawable alias splash_icon that styles.xml shows,
 *   pointing at whichever foreground layer the icon has (vector drawable or mipmap PNGs);
 * - Play listing art in brand/generated/play/: the 512 × 512 icon and the 1024 × 500 feature graphic
 *   (JPEG, no alpha).
 *
 * The vector converter understands what the brand SVGs use — <rect> (with rx), <g fill>,
 * <linearGradient>/<radialGradient> in objectBoundingBox units — and fails on anything else, so a
 * redrawn icon is never converted wrongly in silence. The Android files are written only when
 * web/android/ exists.
 */
import {
	existsSync,
	mkdirSync,
	readFileSync,
	rmSync,
	writeFileSync,
} from "node:fs";
import type { IconRenderer } from "@shared-next/brand/build";

/** Where the Android resources live, relative to web/. */
export const ANDROID_RES_DIR = "android/app/src/main/res/";
/** Play listing art, relative to brand/generated/. */
export const PLAY_DIR = "play/";

const VIEWPORT = 108;
const CANVAS = 1024;
/** Maskable safe circle (radius 40 % of the canvas) → adaptive icon safe zone (radius 33 of 108 dp). */
const ART_SCALE = 33 / (CANVAS * 0.4);

/** Side in pixels of an adaptive icon layer (108 dp) at the density of a 48 dp legacy icon of `legacySize` px. */
export function adaptiveLayerSize(legacySize: number): number {
	return Math.round((legacySize * VIEWPORT) / 48);
}

export const LEGACY_DENSITIES: ReadonlyArray<{ dir: string; size: number }> = [
	{ dir: "mipmap-mdpi", size: 48 },
	{ dir: "mipmap-hdpi", size: 72 },
	{ dir: "mipmap-xhdpi", size: 96 },
	{ dir: "mipmap-xxhdpi", size: 144 },
	{ dir: "mipmap-xxxhdpi", size: 192 },
];

interface Stop {
	readonly offset: number;
	readonly color: string;
}

interface Gradient {
	readonly kind: "linear" | "radial";
	readonly attrs: Readonly<Record<string, string>>;
	readonly stops: readonly Stop[];
}

interface Rect {
	readonly x: number;
	readonly y: number;
	readonly width: number;
	readonly height: number;
	readonly rx: number;
	/** `url(#id)` or a colour. */
	readonly fill: string;
	/** Inside a <g>: artwork (foreground); else full-canvas background. */
	readonly art: boolean;
}

function attributes(tag: string): Record<string, string> {
	const out: Record<string, string> = {};
	for (const match of tag.matchAll(/([\w:-]+)="([^"]*)"/g)) {
		const [, name, value] = match;
		if (name !== undefined && value !== undefined) out[name] = value;
	}
	return out;
}

function numberAttr(
	attrs: Record<string, string>,
	name: string,
	fallback?: number,
): number {
	const raw = attrs[name];
	if (raw === undefined) {
		if (fallback === undefined)
			throw new Error(`brand android: <rect> without ${name}`);
		return fallback;
	}
	const value = Number(raw);
	if (!Number.isFinite(value))
		throw new Error(`brand android: ${name}="${raw}" is not a number`);
	return value;
}

/** `#rrggbb` + opacity → Android `#AARRGGBB`. */
function androidColor(hex: string, opacity = 1): string {
	if (!/^#[0-9a-fA-F]{6}$/.test(hex))
		throw new Error(`brand android: colour ${hex} is not #rrggbb`);
	const alpha = Math.round(Math.min(1, Math.max(0, opacity)) * 255)
		.toString(16)
		.padStart(2, "0");
	return `#${alpha}${hex.slice(1)}`.toUpperCase();
}

function parseSvg(svg: string): {
	gradients: Map<string, Gradient>;
	rects: Rect[];
} {
	const body = svg.replace(/<!--[\s\S]*?-->/g, "");
	const gradients = new Map<string, Gradient>();
	for (const match of body.matchAll(
		/<(linear|radial)Gradient\b([^>]*)>([\s\S]*?)<\/\1Gradient>/g,
	)) {
		const [, kind, head, inner] = match;
		const attrs = attributes(head ?? "");
		if (attrs.id === undefined || (kind !== "linear" && kind !== "radial"))
			continue;
		if (
			attrs.gradientUnits !== undefined &&
			attrs.gradientUnits !== "objectBoundingBox"
		) {
			throw new Error(
				`brand android: gradient ${attrs.id} must use objectBoundingBox units`,
			);
		}
		const stops = [...(inner ?? "").matchAll(/<stop\b[^>]*>/g)].map((stop) => {
			const a = attributes(stop[0]);
			return {
				offset: Number(a.offset ?? "0"),
				color: androidColor(
					a["stop-color"] ?? "#000000",
					Number(a["stop-opacity"] ?? "1"),
				),
			};
		});
		gradients.set(attrs.id, { kind, attrs, stops });
	}
	const drawing = body.replace(/<defs>[\s\S]*?<\/defs>/g, "");
	const rects: Rect[] = [];
	let groupFill: string | undefined;
	for (const match of drawing.matchAll(/<(\/?)(\w+)\b([^>]*?)(\/?)>/g)) {
		const [, closing, name, rest] = match;
		if (name === "svg") continue;
		if (name === "g") {
			groupFill = closing ? undefined : attributes(rest ?? "").fill;
			continue;
		}
		if (name !== "rect")
			throw new Error(`brand android: unsupported <${name}> in the icon SVG`);
		const a = attributes(rest ?? "");
		const fill = a.fill ?? groupFill;
		if (fill === undefined)
			throw new Error("brand android: <rect> without a fill");
		rects.push({
			x: numberAttr(a, "x", 0),
			y: numberAttr(a, "y", 0),
			width: numberAttr(a, "width"),
			height: numberAttr(a, "height"),
			rx: numberAttr(a, "rx", 0),
			fill,
			art: groupFill !== undefined,
		});
	}
	return { gradients, rects };
}

function fmt(value: number): string {
	return Number(value.toFixed(3)).toString();
}

function roundedRectPath(
	x: number,
	y: number,
	w: number,
	h: number,
	radius: number,
): string {
	const r = Math.min(radius, w / 2, h / 2);
	if (r <= 0)
		return `M${fmt(x)},${fmt(y)}H${fmt(x + w)}V${fmt(y + h)}H${fmt(x)}Z`;
	const arc = (ex: number, ey: number) =>
		`A${fmt(r)},${fmt(r)} 0 0 1 ${fmt(ex)},${fmt(ey)}`;
	return [
		`M${fmt(x + r)},${fmt(y)}`,
		`H${fmt(x + w - r)}`,
		arc(x + w, y + r),
		`V${fmt(y + h - r)}`,
		arc(x + w - r, y + h),
		`H${fmt(x + r)}`,
		arc(x, y + h - r),
		`V${fmt(y + r)}`,
		arc(x + r, y),
		"Z",
	].join("");
}

type Mapping = (v: number) => number;

function fillXml(
	rect: Rect,
	gradients: Map<string, Gradient>,
	map: Mapping,
	scale: number,
	monochrome: boolean,
): string {
	if (monochrome) return ' android:fillColor="#FFFFFFFF"/>';
	const ref = /^url\(#([\w-]+)\)$/.exec(rect.fill)?.[1];
	if (ref === undefined)
		return ` android:fillColor="${androidColor(rect.fill)}"/>`;
	const gradient = gradients.get(ref);
	if (gradient === undefined)
		throw new Error(`brand android: gradient #${ref} not defined`);
	const { x, y, width: w, height: h } = rect;
	const frac = (name: string, fallback: number) =>
		Number(gradient.attrs[name] ?? fallback);
	const items = gradient.stops
		.map(
			(stop) =>
				`\n                <item android:offset="${fmt(stop.offset)}" android:color="${stop.color}"/>`,
		)
		.join("");
	let geometry: string;
	if (gradient.kind === "linear") {
		geometry =
			`android:type="linear" android:startX="${fmt(map(x + frac("x1", 0) * w))}" android:startY="${fmt(map(y + frac("y1", 0) * h))}"` +
			` android:endX="${fmt(map(x + frac("x2", 1) * w))}" android:endY="${fmt(map(y + frac("y2", 0) * h))}"`;
	} else {
		if (w !== h)
			throw new Error(
				`brand android: radial gradient #${ref} needs a square shape`,
			);
		geometry =
			`android:type="radial" android:centerX="${fmt(map(x + frac("cx", 0.5) * w))}" android:centerY="${fmt(map(y + frac("cy", 0.5) * h))}"` +
			` android:gradientRadius="${fmt(frac("r", 0.5) * w * scale)}"`;
	}
	return `>
        <aapt:attr name="android:fillColor">
            <gradient ${geometry}>${items}
            </gradient>
        </aapt:attr>
    </path>`;
}

function vectorXml(
	rects: readonly Rect[],
	gradients: Map<string, Gradient>,
	map: Mapping,
	scale: number,
	monochrome: boolean,
): string {
	const paths = rects
		.map((rect) => {
			const d = roundedRectPath(
				map(rect.x),
				map(rect.y),
				rect.width * scale,
				rect.height * scale,
				rect.rx * scale,
			);
			return `    <path android:pathData="${d}"${fillXml(rect, gradients, map, scale, monochrome)}`;
		})
		.join("\n");
	return `<?xml version="1.0" encoding="utf-8"?>
<!-- Generated by pnpm run brand:gen (scripts/brand-android.ts) from the brand icon. Do not edit. -->
<vector xmlns:android="http://schemas.android.com/apk/res/android"
    xmlns:aapt="http://schemas.android.com/aapt"
    android:width="${VIEWPORT}dp"
    android:height="${VIEWPORT}dp"
    android:viewportWidth="${VIEWPORT}"
    android:viewportHeight="${VIEWPORT}">
${paths}
</vector>
`;
}

const ADAPTIVE_ICON = `<?xml version="1.0" encoding="utf-8"?>
<!-- Generated by pnpm run brand:gen (scripts/brand-android.ts). Do not edit. -->
<adaptive-icon xmlns:android="http://schemas.android.com/apk/res/android">
    <background android:drawable="@drawable/ic_launcher_background"/>
    <foreground android:drawable="@drawable/ic_launcher_foreground"/>
    <monochrome android:drawable="@drawable/ic_launcher_monochrome"/>
</adaptive-icon>
`;

/** The adaptive icon of a picture logo: PNG foreground (and themed layer, when there is one) per density. */
function pictureAdaptiveIcon(monochrome: boolean): string {
	const themed = monochrome
		? '\n    <monochrome android:drawable="@mipmap/ic_launcher_monochrome"/>'
		: "";
	return `<?xml version="1.0" encoding="utf-8"?>
<!-- Generated by pnpm run brand:gen (scripts/brand-android.ts). Do not edit. -->
<adaptive-icon xmlns:android="http://schemas.android.com/apk/res/android">
    <background android:drawable="@drawable/ic_launcher_background"/>
    <foreground android:drawable="@mipmap/ic_launcher_foreground"/>${themed}
</adaptive-icon>
`;
}

/**
 * The themed-icon silhouette of a picture logo, relative to web/ (not a
 * brand.config.json field: the shared schema has none). Derived once from the
 * AI master: white symbol on transparent, the same framing as the maskable icon.
 */
export const MONOCHROME_ICON = "brand/icon-monochrome.svg";

/** brand/icon-monochrome.svg under web/, or null when there is none. */
export function readMonochromeIcon(root: URL): string | null {
	const file = new URL(MONOCHROME_ICON, root);
	return existsSync(file) ? readFileSync(file, "utf8") : null;
}

/** Whether the icon SVG holds a picture (an embedded <image>) instead of shapes the vector converter reads. */
export function isPictureIcon(svg: string): boolean {
	return /<image\b/.test(svg.replace(/<!--[\s\S]*?-->/g, ""));
}

/** Drawables of the vector adaptive icon that a picture icon replaces with mipmap PNGs or drops. */
export const VECTOR_ONLY_DRAWABLES = [
	"drawable/ic_launcher_foreground.xml",
	"drawable/ic_launcher_monochrome.xml",
] as const;

/** The maskable picture centred on a transparent 108 dp layer, its safe circle on the adaptive safe zone. */
export function pictureForegroundSvg(maskableSvg: string): string {
	const side = CANVAS * ART_SCALE;
	const offset = (VIEWPORT - side) / 2;
	const data = Buffer.from(maskableSvg).toString("base64");
	return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${VIEWPORT} ${VIEWPORT}"><image x="${fmt(offset)}" y="${fmt(offset)}" width="${fmt(side)}" height="${fmt(side)}" href="data:image/svg+xml;base64,${data}"/></svg>`;
}

function colorsXml(colors: Record<string, string>): string {
	const items = Object.entries(colors)
		.map(([name, value]) => `    <color name="${name}">${value}</color>`)
		.join("\n");
	return `<?xml version="1.0" encoding="utf-8"?>
<!-- Generated by pnpm run brand:gen (scripts/brand-android.ts) from brand.config.json colors. Do not edit. -->
<resources>
${items}
</resources>
`;
}

/** The splash icon of styles.xml (@drawable/splash_icon): an alias of the adaptive foreground layer. */
function splashIconXml(foreground: string): string {
	return `<?xml version="1.0" encoding="utf-8"?>
<!-- Generated by pnpm run brand:gen (scripts/brand-android.ts): the launcher foreground layer. Do not edit. -->
<resources>
    <item name="splash_icon" type="drawable">${foreground}</item>
</resources>
`;
}

export interface AndroidBrandColors {
	readonly primary: string;
	readonly background: string;
	readonly backgroundDark: string;
}

/**
 * Text resources (path under res/ → content) derived from the maskable icon SVG and the brand colours;
 * `monochromeSvg` (brand/icon-monochrome.svg, or null) gives a picture icon its themed layer.
 */
export function androidResourceFiles(
	maskableSvg: string,
	colors: AndroidBrandColors,
	monochromeSvg: string | null,
): Record<string, string> {
	const brandColors = {
		"values/brand_colors.xml": colorsXml({
			splash_background: colors.background,
			ic_launcher_background: colors.backgroundDark,
		}),
		"values-night/brand_colors.xml": colorsXml({
			splash_background: colors.backgroundDark,
			ic_launcher_background: colors.backgroundDark,
		}),
	};
	if (isPictureIcon(maskableSvg)) {
		const canvas: Rect = {
			x: 0,
			y: 0,
			width: CANVAS,
			height: CANVAS,
			rx: 0,
			fill: colors.primary,
			art: false,
		};
		const scale = VIEWPORT / CANVAS;
		return {
			"drawable/ic_launcher_background.xml": vectorXml(
				[canvas],
				new Map(),
				(v) => v * scale,
				scale,
				false,
			),
			"mipmap-anydpi-v26/ic_launcher.xml": pictureAdaptiveIcon(
				monochromeSvg !== null,
			),
			"mipmap-anydpi-v26/ic_launcher_round.xml": pictureAdaptiveIcon(
				monochromeSvg !== null,
			),
			"values/brand_splash.xml": splashIconXml(
				"@mipmap/ic_launcher_foreground",
			),
			...brandColors,
		};
	}
	const { gradients, rects } = parseSvg(maskableSvg);
	const background = rects.filter((rect) => !rect.art);
	const art = rects.filter((rect) => rect.art);
	if (background.length === 0 || art.length === 0) {
		throw new Error(
			"brand android: the icon needs full-canvas background rects and <g> artwork",
		);
	}
	const fullScale = VIEWPORT / CANVAS;
	const toBackground: Mapping = (v) => v * fullScale;
	const toArt: Mapping = (v) => VIEWPORT / 2 + (v - CANVAS / 2) * ART_SCALE;
	return {
		"drawable/ic_launcher_background.xml": vectorXml(
			background,
			gradients,
			toBackground,
			fullScale,
			false,
		),
		"drawable/ic_launcher_foreground.xml": vectorXml(
			art,
			gradients,
			toArt,
			ART_SCALE,
			false,
		),
		"drawable/ic_launcher_monochrome.xml": vectorXml(
			art,
			gradients,
			toArt,
			ART_SCALE,
			true,
		),
		"mipmap-anydpi-v26/ic_launcher.xml": ADAPTIVE_ICON,
		"mipmap-anydpi-v26/ic_launcher_round.xml": ADAPTIVE_ICON,
		"values/brand_splash.xml": splashIconXml(
			"@drawable/ic_launcher_foreground",
		),
		...brandColors,
	};
}

/** The maskable SVG clipped to a circle: the legacy round launcher icon. */
export function roundIconSvg(maskableSvg: string): string {
	const inner = maskableSvg.replace(/<\?xml[^>]*>/, "");
	return `<svg xmlns="http://www.w3.org/2000/svg" width="${CANVAS}" height="${CANVAS}" viewBox="0 0 ${CANVAS} ${CANVAS}"><defs><clipPath id="android-round"><circle cx="${CANVAS / 2}" cy="${CANVAS / 2}" r="${CANVAS / 2}"/></clipPath></defs><g clip-path="url(#android-round)">${inner}</g></svg>`;
}

export interface WriteAndroidBrandOptions {
	/** web/ */
	readonly root: URL;
	/** brand/generated/ */
	readonly generatedDir: URL;
	readonly iconSvg: string;
	readonly maskableSvg: string;
	/** brand/icon-monochrome.svg (readMonochromeIcon), or null: the themed layer of a picture icon. */
	readonly monochromeSvg: string | null;
	readonly appName: string;
	readonly colors: AndroidBrandColors;
	readonly renderer: IconRenderer;
	/** Playwright's `chromium`, for the feature graphic (not square, so not the icon renderer). */
	readonly chromium: FeatureLauncher;
}

interface FeatureLauncher {
	launch(): Promise<{
		newPage(options: { deviceScaleFactor: number }): Promise<FeaturePage>;
		close(): Promise<void>;
	}>;
}

interface FeaturePage {
	setViewportSize(size: { width: number; height: number }): Promise<void>;
	setContent(html: string): Promise<void>;
	evaluate(expression: string): Promise<unknown>;
	screenshot(options: {
		type: "jpeg";
		quality: number;
		clip: { x: number; y: number; width: number; height: number };
	}): Promise<Uint8Array>;
}

function writeFile(url: URL, content: string | Uint8Array): void {
	mkdirSync(new URL("./", url), { recursive: true });
	writeFileSync(url, content);
}

function escapeHtml(value: string): string {
	return value.replace(
		/[&<>"]/g,
		(char) =>
			({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[char] ?? char,
	);
}

/** 1024 × 500, no alpha: the brand icon and name on the brand's dark background. */
async function featureGraphic(
	options: WriteAndroidBrandOptions,
): Promise<Uint8Array> {
	const font = new URL(
		"node_modules/@fontsource/inter/files/inter-latin-700-normal.woff2",
		options.root,
	);
	const fontFace = existsSync(font)
		? `@font-face{font-family:BrandFont;font-weight:700;src:url(data:font/woff2;base64,${readFileSync(font).toString("base64")}) format("woff2")}`
		: "";
	const icon = Buffer.from(options.iconSvg).toString("base64");
	const html = `<html><head><style>${fontFace}
html,body{margin:0;width:1024px;height:500px;background:${options.colors.backgroundDark}}
.frame{display:flex;align-items:center;justify-content:center;gap:56px;width:1024px;height:500px;
background:radial-gradient(circle at 30% 50%, ${options.colors.primary}55, transparent 60%)}
img{width:240px;height:240px}
span{font:700 128px BrandFont, Inter, system-ui, sans-serif;color:#ffffff;letter-spacing:-2px}
</style></head><body><div class="frame"><img id="i" src="data:image/svg+xml;base64,${icon}"><span>${escapeHtml(options.appName)}</span></div></body></html>`;
	const browser = await options.chromium.launch();
	try {
		const page = await browser.newPage({ deviceScaleFactor: 1 });
		await page.setViewportSize({ width: 1024, height: 500 });
		await page.setContent(html);
		await page.evaluate(
			"document.fonts.ready.then(() => document.getElementById('i').decode())",
		);
		return await page.screenshot({
			type: "jpeg",
			quality: 92,
			clip: { x: 0, y: 0, width: 1024, height: 500 },
		});
	} finally {
		await browser.close();
	}
}

/** Writes the Play art always and the Android resources when web/android/ exists; returns the paths written. */
export async function writeAndroidBrandFiles(
	options: WriteAndroidBrandOptions,
): Promise<string[]> {
	const written: string[] = [];
	const playDir = new URL(PLAY_DIR, options.generatedDir);
	const [playIcon, ...legacy] = await options.renderer([
		{ path: "iconMaskable", svg: Buffer.from(options.maskableSvg), size: 512 },
		...LEGACY_DENSITIES.flatMap(({ size }) => [
			{ path: "icon", svg: Buffer.from(options.iconSvg), size },
			{
				path: "iconMaskable (round)",
				svg: Buffer.from(roundIconSvg(options.maskableSvg)),
				size,
			},
		]),
	]);
	if (playIcon === undefined)
		throw new Error("brand android: renderer returned no Play icon");
	writeFile(new URL("icon-512.png", playDir), playIcon);
	writeFile(
		new URL("feature-graphic.jpg", playDir),
		await featureGraphic(options),
	);
	written.push(`${PLAY_DIR}icon-512.png`, `${PLAY_DIR}feature-graphic.jpg`);

	const resDir = new URL(ANDROID_RES_DIR, options.root);
	if (!existsSync(resDir)) return written;
	if (isPictureIcon(options.maskableSvg)) {
		const layerSvgs = [
			{
				file: "ic_launcher_foreground.png",
				label: "iconMaskable (adaptive foreground)",
				svg: options.maskableSvg,
			},
			...(options.monochromeSvg === null
				? []
				: [
						{
							file: "ic_launcher_monochrome.png",
							label: "icon-monochrome (adaptive themed layer)",
							svg: options.monochromeSvg,
						},
					]),
		];
		const layers = await options.renderer(
			layerSvgs.flatMap(({ label, svg }) =>
				LEGACY_DENSITIES.map(({ size }) => ({
					path: label,
					svg: Buffer.from(pictureForegroundSvg(svg)),
					size: adaptiveLayerSize(size),
				})),
			),
		);
		layerSvgs.forEach(({ file }, j) => {
			LEGACY_DENSITIES.forEach(({ dir }, i) => {
				const layer = layers[j * LEGACY_DENSITIES.length + i];
				if (layer === undefined)
					throw new Error("brand android: renderer returned too few layers");
				writeFile(new URL(`${dir}/${file}`, resDir), layer);
				written.push(`${ANDROID_RES_DIR}${dir}/${file}`);
			});
		});
		const stale = [
			...VECTOR_ONLY_DRAWABLES,
			...(options.monochromeSvg === null
				? LEGACY_DENSITIES.map(({ dir }) => `${dir}/ic_launcher_monochrome.png`)
				: []),
		];
		for (const path of stale) rmSync(new URL(path, resDir), { force: true });
	}
	for (const [path, content] of Object.entries(
		androidResourceFiles(
			options.maskableSvg,
			options.colors,
			options.monochromeSvg,
		),
	)) {
		writeFile(new URL(path, resDir), content);
		written.push(ANDROID_RES_DIR + path);
	}
	LEGACY_DENSITIES.forEach(({ dir }, i) => {
		const square = legacy[i * 2];
		const round = legacy[i * 2 + 1];
		if (square === undefined || round === undefined)
			throw new Error("brand android: renderer returned too few icons");
		writeFile(new URL(`${dir}/ic_launcher.png`, resDir), square);
		writeFile(new URL(`${dir}/ic_launcher_round.png`, resDir), round);
		written.push(
			`${ANDROID_RES_DIR}${dir}/ic_launcher.png`,
			`${ANDROID_RES_DIR}${dir}/ic_launcher_round.png`,
		);
	});
	return written;
}
