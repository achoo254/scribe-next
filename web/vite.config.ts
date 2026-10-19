import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig, loadEnv } from "vite";

/** Build-time config: the backend origin the native shell calls. */
const REQUIRED_ENV = ["VITE_NATIVE_API_ORIGIN"] as const;

/** Vendor chunks: libraries stay cached across deploys. */
const VENDOR_CHUNKS: ReadonlyArray<readonly [string, RegExp]> = [
	["vendor-react", /^(?:react|react-dom|scheduler|react-router|cookie)$/],
	["vendor-query", /^@tanstack\//],
	["vendor-radix", /^@radix-ui\//],
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

/** True for `https://host[:port]` with nothing after the origin. */
function isHttpOrigin(value: string): boolean {
	try {
		const url = new URL(value);
		return (
			(url.protocol === "https:" || url.protocol === "http:") &&
			url.origin === value.replace(/\/$/, "")
		);
	} catch {
		return false;
	}
}

/** Same rule as `supportEmailOf` in src/lib/legal.ts. */
function isSupportEmail(value: string): boolean {
	return /^[^\s@<>]+@[^\s@<>]+\.[^\s@<>]+$/.test(value);
}

export default defineConfig(({ command, mode }) => {
	// Without VITE_NATIVE_API_ORIGIN the iPhone app has no backend; fail the build
	// instead. Read from web/.env (not in git, copy web/.env.example; see
	// docs/deploy-web-production.md).
	if (command === "build") {
		const env = loadEnv(mode, process.cwd(), "VITE_");
		const missing = REQUIRED_ENV.filter((name) => !env[name]);
		if (missing.length > 0) {
			throw new Error(
				`Missing ${missing.join(", ")} for mode "${mode}": create web/.env from web/.env.example.`,
			);
		}
		// Optional (src/lib/idp.ts has the default), but a typo must not ship
		// account links that silently point at the default IdP.
		const idp = env.VITE_IDP_ORIGIN;
		if (idp && !isHttpOrigin(idp)) {
			throw new Error(
				`VITE_IDP_ORIGIN must be an http(s) origin such as https://auth.hoandat.com, got "${idp}".`,
			);
		}
		// Optional too; a typo must not ship a broken contact on the legal pages.
		const support = env.VITE_SUPPORT_EMAIL;
		if (support && !isSupportEmail(support)) {
			throw new Error(
				`VITE_SUPPORT_EMAIL must be an email address, got "${support}".`,
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
				// xfwd: BE reads the client IP from X-Forwarded-For, as behind nginx.
				"/api": {
					target: "http://127.0.0.1:8787",
					changeOrigin: true,
					xfwd: true,
				},
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
