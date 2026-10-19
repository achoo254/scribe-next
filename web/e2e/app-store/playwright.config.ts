import { defineConfig, devices } from "@playwright/test";
import { E2E_ORIGIN, E2E_PORT } from "../support/e2e-origin";

/**
 * App Store screenshots (`pnpm run screenshots:app-store`), not a test run:
 * the iOS app's UI over the stubbed API with demo data, saved as PNG under
 * docs/app-store/screenshots/<size>/. One project per display class App
 * Store Connect takes, at its exact pixel size (CSS viewport × 3).
 */
const IPHONE = devices["iPhone 15"];

export default defineConfig({
	testDir: ".",
	testMatch: "screenshots.spec.ts",
	outputDir: "../test-results/app-store",
	workers: 1,
	retries: 0,
	reporter: "list",
	timeout: 60_000,
	use: {
		baseURL: E2E_ORIGIN,
		testIdAttribute: "data-id",
		locale: "vi-VN",
		timezoneId: "Asia/Saigon",
	},
	projects: [
		// iPhone with Dynamic Island, large (6.9"): 1320 × 2868.
		{
			name: "6.9",
			use: {
				...IPHONE,
				viewport: { width: 440, height: 956 },
				deviceScaleFactor: 3,
			},
		},
		// iPhone with Face ID, large (6.5"): 1284 × 2778.
		{
			name: "6.5",
			use: {
				...IPHONE,
				viewport: { width: 428, height: 926 },
				deviceScaleFactor: 3,
			},
		},
		// iPhone with Dynamic Island, medium (6.3"): 1206 × 2622.
		{
			name: "6.3",
			use: {
				...IPHONE,
				viewport: { width: 402, height: 874 },
				deviceScaleFactor: 3,
			},
		},
	],
	webServer: {
		command: `pnpm exec vite --host 127.0.0.1 --port ${E2E_PORT} --strictPort`,
		cwd: "../..",
		url: E2E_ORIGIN,
		reuseExistingServer: false,
		timeout: 60_000,
		env: { VITE_NATIVE_API_ORIGIN: E2E_ORIGIN },
	},
});
