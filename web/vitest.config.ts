import { defineConfig } from "vitest/config";

// No Vite plugins here: esbuild's automatic JSX runtime is all tests need.
export default defineConfig({
	test: {
		projects: [
			{
				esbuild: { jsx: "automatic" },
				test: {
					name: "web",
					environment: "jsdom",
					setupFiles: ["./src/test/setup.ts"],
					include: ["src/**/*.test.{ts,tsx}"],
					css: false,
				},
			},
			{
				test: {
					name: "scripts",
					environment: "node",
					include: ["scripts/**/*.test.ts"],
				},
			},
		],
	},
});
