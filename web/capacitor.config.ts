import type { CapacitorConfig } from "@capacitor/cli";

/**
 * iOS shell around web/dist (plan 260728-1609, D3): the page runs at
 * capacitor://localhost and CapacitorHttp sends every fetch through
 * URLSession, so calls to the backend are not blocked by CORS or the
 * SameSite=Lax session cookie. lib/api.ts adds the backend origin when
 * running natively.
 */
const config: CapacitorConfig = {
	appId: "vn.inet.onescribe",
	appName: "OneScribe",
	webDir: "dist",
	plugins: {
		CapacitorHttp: { enabled: true },
	},
};

export default config;
