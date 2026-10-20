/*
 * Copied from chat-next/packages/brand @a5be0b4; keep in sync until shared
 * package exists. Scribe changes: no Tauri manifest (no desktop app), the
 * Capacitor manifest holds only what capacitor.config.ts spreads, and the
 * web manifest's theme colour is the page background, as the
 * <meta name="theme-color"> that src/lib/theme.ts keeps on --c-bg.
 */
import type { BrandConfig } from "./config.js";
import { manifestIcons } from "./icons.js";

/** Platform manifests derived from the brand, written to brand/generated/. */
export interface GeneratedManifests {
	readonly "web.webmanifest.json": Record<string, unknown>;
	readonly "capacitor.json": {
		readonly appId: string;
		readonly appName: string;
	};
}

export function generateManifests(brand: BrandConfig): GeneratedManifests {
	return {
		"web.webmanifest.json": {
			// A stable identity: browsers treat the installed app as the same one across start_url changes.
			id: "/",
			name: brand.appName,
			short_name: brand.shortName,
			description: brand.description,
			lang: "vi",
			dir: "ltr",
			start_url: "/",
			scope: "/",
			display: "standalone",
			theme_color: brand.colors.background,
			background_color: brand.colors.background,
			categories: ["productivity"],
			prefer_related_applications: false,
			icons: manifestIcons(),
		},
		// capacitor.config.ts adds webDir and plugins. No server.hostname: the
		// WebView keeps capacitor://localhost (lib/api.ts adds the backend origin).
		"capacitor.json": {
			appId: brand.appIds.ios,
			appName: brand.appName,
		},
	};
}

/** Text of a generated JSON file; the same brand always gives the same bytes. */
export function formatGenerated(content: unknown): string {
	return `${JSON.stringify(content, null, 2)}\n`;
}
