import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig, loadEnv } from "vite";

/** Firebase web config (public, not a secret) read by src/lib/firebase.ts. */
const REQUIRED_ENV = [
	"VITE_FIREBASE_API_KEY",
	"VITE_FIREBASE_AUTH_DOMAIN",
	"VITE_FIREBASE_PROJECT_ID",
] as const;

/** Vendor chunks: libraries stay cached across deploys. */
const VENDOR_CHUNKS: ReadonlyArray<readonly [string, RegExp]> = [
	["vendor-react", /^(?:react|react-dom|scheduler|react-router|cookie)$/],
	["vendor-query", /^@tanstack\//],
	["vendor-radix", /^@radix-ui\//],
	["vendor-firebase", /^(?:firebase|@firebase\/)/],
];

const PACKAGES_DIR = "/node_modules/";

/** Package name of a module inside the installed packages directory. */
function packageOf(id: string): string | null {
	const index = id.lastIndexOf(PACKAGES_DIR);
	if (index === -1) return null;
	const [scope = "", name = ""] = id
		.slice(index + PACKAGES_DIR.length)
		.split("/");
	return scope.startsWith("@") ? `${scope}/${name}` : scope;
}

function vendorChunk(id: string): string | undefined {
	const name = packageOf(id);
	if (name === null) return undefined;
	return VENDOR_CHUNKS.find(([, pattern]) => pattern.test(name))?.[0];
}

export default defineConfig(({ command, mode }) => {
	// Without the Firebase config the built app shows a blank page
	// (auth/invalid-api-key); fail the build instead. Dev and prod use the
	// same Firebase project, read from web/.env (not in git, see
	// docs/deploy-web-production.md).
	if (command === "build") {
		const env = loadEnv(mode, process.cwd(), "VITE_");
		const missing = REQUIRED_ENV.filter((name) => !env[name]);
		if (missing.length > 0) {
			throw new Error(
				`Missing ${missing.join(", ")} for mode "${mode}": create web/.env from web/.env.example.`,
			);
		}
	}
	return {
		plugins: [react(), tailwindcss()],
		server: {
			port: 5173,
			strictPort: true,
			// Same origin as production behind nginx: FE and BE share the
			// session cookie without CORS.
			proxy: {
				"/api": { target: "http://127.0.0.1:8787", changeOrigin: true },
			},
		},
		build: {
			// Fonts stay separate files instead of base64 in the stylesheet.
			assetsInlineLimit: (file) =>
				file.endsWith(".woff2") ? false : undefined,
			rollupOptions: { output: { manualChunks: vendorChunk } },
		},
	};
});
