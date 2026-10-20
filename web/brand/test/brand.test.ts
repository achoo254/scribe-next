/*
 * Adapted from chat-next/packages/brand/test/brand.test.ts @a5be0b4; keep in
 * sync until shared package exists. Scribe adds: generated files and the
 * native config follow the brand, and the brand colours are the tokens.
 */
import { existsSync, readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import capacitorConfig from "../../capacitor.config.js";
import { resolveThemes } from "../../scripts/check-tokens/parse-tokens.js";
import {
	FAVICON_ICO,
	encodeIco,
	formatGenerated,
	generateManifests,
	iconSourceHashes,
	loadBrandConfig,
	manifestIcons,
	parseBrandConfig,
	webIconFiles,
} from "../src/index.js";

const brand = loadBrandConfig();
const generated = (name: string) =>
	readFileSync(new URL(`../generated/${name}`, import.meta.url), "utf8");

describe("brand.config.json", () => {
	it("passes its schema", () => {
		expect(brand.urlScheme).toBe(brand.appIds.ios);
	});

	it("rejects an invalid bundle id, color and unknown field", () => {
		expect(() =>
			parseBrandConfig({
				...brand,
				appIds: { ...brand.appIds, ios: "Not A Bundle" },
				colors: { ...brand.colors, primary: "blue" },
				extra: true,
			}),
		).toThrow(/appIds\/ios.*colors\/primary|colors\/primary.*appIds\/ios/s);
	});

	it("uses the colours of src/styles/tokens.css", () => {
		const css = readFileSync(
			new URL("../../src/styles/tokens.css", import.meta.url),
			"utf8",
		);
		const themes = resolveThemes(css);
		expect(brand.colors.primary).toBe(themes.light.get("--c-primary-solid"));
		expect(brand.colors.background).toBe(themes.light.get("--c-bg"));
		expect(brand.colors.backgroundDark).toBe(themes.dark.get("--c-bg"));
	});
});

describe("manifests", () => {
	it("follow the brand, so renaming means editing brand.config.json only", () => {
		const renamed = generateManifests({
			...brand,
			appName: "Renamed",
			shortName: "Ren",
		});
		expect(renamed["web.webmanifest.json"]).toMatchObject({
			name: "Renamed",
			short_name: "Ren",
		});
		expect(renamed["capacitor.json"]).toMatchObject({ appName: "Renamed" });
	});

	it("are on disk as generated from the current brand (run pnpm brand:gen after editing it)", () => {
		for (const [name, content] of Object.entries(generateManifests(brand))) {
			expect(generated(name), name).toBe(formatGenerated(content));
		}
	});

	it("give the iPhone app its bundle id and name, and nothing else changes", () => {
		expect(capacitorConfig).toEqual({
			appId: brand.appIds.ios,
			appName: brand.appName,
			webDir: "dist",
			plugins: { CapacitorHttp: { enabled: true } },
		});
	});

	it("keep the Capacitor WebView on its own origin, not a tenant's host", () => {
		expect(capacitorConfig).not.toHaveProperty("server");
	});
});

describe("icons", () => {
	it("were generated from the current brand SVGs (run pnpm brand:gen after editing them)", () => {
		const recorded: unknown = JSON.parse(generated("icons/icons-source.json"));
		expect(recorded).toEqual(iconSourceHashes(brand));
	});

	it("has a file on disk for every icon the web app serves and the manifest lists", () => {
		const served = webIconFiles(brand);
		for (const icon of served)
			expect(existsSync(icon.file), icon.path).toBe(true);
		const paths = new Set(served.map((icon) => `/${icon.path}`));
		for (const icon of manifestIcons()) {
			expect(paths.has(icon.src ?? "")).toBe(true);
		}
		expect(manifestIcons().some((icon) => icon.purpose === "maskable")).toBe(
			true,
		);
	});

	it("packs PNGs into an ICO directory with sizes and offsets", () => {
		const a = new Uint8Array([1, 2, 3]);
		const b = new Uint8Array([4, 5]);
		const ico = encodeIco([
			{ size: 16, png: a },
			{ size: 256, png: b },
		]);
		const view = new DataView(ico.buffer);
		expect(view.getUint16(2, true)).toBe(1);
		expect(view.getUint16(4, true)).toBe(2);
		expect(ico[6]).toBe(16);
		expect(ico[22]).toBe(0); // 256 is written as 0
		expect(view.getUint32(14, true)).toBe(3);
		expect(view.getUint32(18, true)).toBe(38);
		expect(view.getUint32(34, true)).toBe(41);
		expect([...ico.slice(38)]).toEqual([1, 2, 3, 4, 5]);
		expect(() => encodeIco([])).toThrow();
		expect(() => encodeIco([{ size: 300, png: a }])).toThrow(/1..256/);
	});

	it("keeps favicon.ico readable as an icon file", () => {
		const ico = readFileSync(
			new URL(`../generated/icons/${FAVICON_ICO}`, import.meta.url),
		);
		expect(ico.readUInt16LE(2)).toBe(1);
		expect(ico.readUInt16LE(4)).toBe(3);
	});
});
