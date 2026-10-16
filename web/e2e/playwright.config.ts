import { defineConfig, devices } from "@playwright/test";
import { E2E_ORIGIN, E2E_PORT } from "./support/e2e-origin";

/**
 * Browser specs against the Vite dev server with every `/api` call stubbed
 * (support/api-stubs.ts): no backend, no account. Screenshots are the
 * baseline for the UI refactor; they are made on macOS (file suffix
 * `-darwin`) and change only on purpose (`--update-snapshots`).
 */
export default defineConfig({
	testDir: "./specs",
	outputDir: "./test-results",
	fullyParallel: true,
	// Shared 16 GB machine: two browsers at a time.
	workers: 2,
	retries: 0,
	forbidOnly: Boolean(process.env.CI),
	reporter: "list",
	timeout: 30_000,
	expect: {
		timeout: 10_000,
		toHaveScreenshot: { maxDiffPixelRatio: 0.002 },
	},
	use: {
		baseURL: E2E_ORIGIN,
		// The app's own test ids are `data-id`, as in every component.
		testIdAttribute: "data-id",
		locale: "vi-VN",
		timezoneId: "Asia/Saigon",
		trace: "retain-on-failure",
		screenshot: "only-on-failure",
	},
	projects: [
		{ name: "chromium-desktop", use: { ...devices["Desktop Chrome"] } },
		{ name: "webkit-iphone", use: { ...devices["iPhone 15"] } },
	],
	webServer: {
		command: `npx vite --host 127.0.0.1 --port ${E2E_PORT} --strictPort`,
		cwd: "..",
		url: E2E_ORIGIN,
		// Never attach to someone else's server on this port.
		reuseExistingServer: false,
		timeout: 60_000,
		// The native shell calls this origin instead of production
		// (VITE_NATIVE_API_ORIGIN in web/.env), so the stubs answer it too.
		env: { VITE_NATIVE_API_ORIGIN: E2E_ORIGIN },
	},
});
