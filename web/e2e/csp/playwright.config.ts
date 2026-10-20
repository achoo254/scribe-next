import { defineConfig, devices } from "@playwright/test";
import baseConfig from "../playwright.config";
import { E2E_ORIGIN } from "../support/e2e-origin";

/**
 * `pnpm run test:e2e:csp`: the production build (`web/dist`, build it
 * first) served with the production security headers of
 * deploy/nginx-scribe-security-headers.inc, running csp.spec.ts and every
 * browser spec of e2e/specs. Any CSP violation fails the run (csp.spec.ts
 * in the page, csp-teardown.ts for the other specs).
 *
 * Browser UI only, in Chromium and WebKit: the iOS app's UI (Capacitor
 * stub) runs from the app bundle, never under these headers. Screenshots
 * are not compared: this run is about the headers, the main run
 * (`pnpm run test:e2e`) owns the baselines.
 */
export default defineConfig({
	...baseConfig,
	testDir: "..",
	testMatch: ["specs/**/*.spec.ts", "csp/**/*.spec.ts"],
	outputDir: "../test-results/csp",
	ignoreSnapshots: true,
	globalTeardown: "./csp-teardown.ts",
	projects: [
		{
			name: "chromium-desktop",
			use: {
				...devices["Desktop Chrome"],
				launchOptions: {
					args: [
						"--use-fake-device-for-media-stream",
						"--use-fake-ui-for-media-stream",
					],
				},
			},
		},
		{ name: "webkit-desktop", use: { ...devices["Desktop Safari"] } },
	],
	webServer: {
		command: "pnpm exec tsx e2e/csp/serve-dist.ts",
		cwd: "../..",
		url: E2E_ORIGIN,
		reuseExistingServer: false,
		timeout: 30_000,
	},
});
