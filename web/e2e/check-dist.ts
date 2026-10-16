/**
 * `npm run check:dist` (also the last step of `npm run build`): the
 * production build must not contain the e2e stubs (API stubs, Capacitor
 * stub). Fails when any stub marker from support/stub-markers.ts appears
 * in `dist/`, or when there is no build to check.
 */
import { readFile, readdir } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { E2E_STUB_MARKERS } from "./support/stub-markers";

const DIST = path.resolve(
	path.dirname(fileURLToPath(import.meta.url)),
	"../dist",
);
const TEXT_EXTENSIONS = new Set([".js", ".mjs", ".css", ".html", ".json"]);

async function listFiles(dir: string): Promise<string[]> {
	const entries = await readdir(dir, { withFileTypes: true });
	const nested = await Promise.all(
		entries.map(async (entry) => {
			const full = path.join(dir, entry.name);
			if (entry.isDirectory()) return listFiles(full);
			return TEXT_EXTENSIONS.has(path.extname(entry.name)) ? [full] : [];
		}),
	);
	return nested.flat();
}

async function main(): Promise<number> {
	const files = await listFiles(DIST).catch((error: unknown) => {
		throw new Error(`No build at ${DIST}: run npm run build first`, {
			cause: error,
		});
	});
	if (!files.some((file) => file.endsWith(".js"))) {
		console.error(`✗ ${DIST} has no JavaScript: run npm run build first`);
		return 1;
	}
	const leaks: string[] = [];
	for (const file of files) {
		const text = await readFile(file, "utf8");
		for (const marker of E2E_STUB_MARKERS) {
			if (text.includes(marker)) {
				leaks.push(`${path.relative(DIST, file)}: ${marker}`);
			}
		}
	}
	if (leaks.length > 0) {
		console.error(`✗ e2e stub code in the production build (${DIST}):`);
		for (const leak of leaks) console.error(`  ${leak}`);
		return 1;
	}
	console.log(
		`✓ No e2e stub code in dist (${files.length} files checked for ${E2E_STUB_MARKERS.join(", ")})`,
	);
	return 0;
}

main().then(
	(code) => {
		process.exitCode = code;
	},
	(error: unknown) => {
		console.error("check:dist crashed:", error);
		process.exitCode = 1;
	},
);
