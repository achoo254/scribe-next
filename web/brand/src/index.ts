/*
 * Copied from chat-next/packages/brand @a5be0b4; keep in sync until shared
 * package exists. Node-side brand API (build config, generator, tests); the
 * browser reads brand.config.json through src/lib/brand.ts.
 */
export {
	type BrandConfig,
	loadBrandConfig,
	parseBrandConfig,
} from "./config.js";
export {
	type GeneratedManifests,
	formatGenerated,
	generateManifests,
} from "./generate.js";
export {
	FAVICON_ICO,
	FAVICON_SVG,
	RASTER_ICONS,
	type RasterIcon,
	type WebIconFile,
	encodeIco,
	iconSourceHashes,
	manifestIcons,
	webIconFiles,
} from "./icons.js";
