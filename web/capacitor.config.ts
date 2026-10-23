import type { CapacitorConfig } from "@capacitor/cli";
// appId and appName come from brand.config.json through `pnpm brand:gen`.
import brand from "./brand/generated/capacitor.json" with { type: "json" };

/**
 * iOS and Android shells around web/dist (plan 260728-1609, D3): the page
 * runs at capacitor://localhost (iOS) or https://localhost (Android) and
 * CapacitorHttp sends every fetch through URLSession / HttpURLConnection,
 * so calls to the backend are not blocked by CORS or the SameSite=Lax
 * session cookie. lib/api.ts adds the backend origin when running natively.
 */
const config: CapacitorConfig = {
	appId: brand.appId,
	appName: brand.appName,
	webDir: "dist",
	plugins: {
		CapacitorHttp: { enabled: true },
	},
};

export default config;
