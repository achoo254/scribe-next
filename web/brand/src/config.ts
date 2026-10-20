/*
 * Copied from chat-next/packages/brand @a5be0b4; keep in sync until shared
 * package exists. Scribe changes: the brand files live in web/ (the only
 * pnpm project; web/ios is inside it), and the fields are the ones Scribe
 * ships (iOS only, no shared media domains, no push sender name).
 */
import { readFileSync } from "node:fs";
import { Ajv2020 } from "ajv/dist/2020.js";

/** web/brand.config.json, as described by web/brand.config.schema.json. */
export interface BrandConfig {
	readonly appName: string;
	readonly shortName: string;
	readonly description: string;
	readonly colors: {
		readonly primary: string;
		readonly background: string;
		readonly backgroundDark: string;
	};
	readonly icon: string;
	readonly iconMaskable: string;
	readonly appIds: {
		readonly ios: string;
	};
	readonly urlScheme: string;
	readonly primaryTenant: { readonly domain: string; readonly issuer: string };
	readonly hostTemplates: {
		readonly web: string;
	};
}

/** web/, where brand.config.json and the paths inside it are rooted. */
export const WEB_ROOT = new URL("../../", import.meta.url);

function readJson(url: URL): unknown {
	return JSON.parse(readFileSync(url, "utf8"));
}

/**
 * Validates `value` against web/brand.config.schema.json and returns it
 * typed, or throws with every schema violation listed.
 */
export function parseBrandConfig(value: unknown): BrandConfig {
	const schema = readJson(new URL("brand.config.schema.json", WEB_ROOT));
	const ajv = new Ajv2020({ allErrors: true, strict: true });
	const validate = ajv.compile<BrandConfig>(schema as object);
	if (!validate(value)) {
		const details = (validate.errors ?? [])
			.map((error) => `${error.instancePath || "/"} ${error.message ?? ""}`)
			.join("; ");
		throw new Error(`brand.config.json is invalid: ${details}`);
	}
	return value;
}

/** Reads and validates web/brand.config.json. */
export function loadBrandConfig(): BrandConfig {
	return parseBrandConfig(readJson(new URL("brand.config.json", WEB_ROOT)));
}
