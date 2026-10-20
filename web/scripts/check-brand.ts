/**
 * `pnpm run check:brand` (part of `pnpm run lint`): fails when a product
 * name is hard-coded in web sources instead of read from brand.config.json.
 * Rules: scripts/check-brand/find-problems.ts. Native code under ios/ is
 * not scanned: the iPhone app keeps its names (Info.plist, Keychain
 * services) as shipped.
 */
import { readFile, readdir } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
	type BrandProblem,
	findBrandProblems,
} from "./check-brand/find-problems.js";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const SCANNED_DIRS = ["src", "scripts", "e2e", "brand/src", "brand/test"];
const SCANNED_FILES = [
	"index.html",
	"vite.config.ts",
	"vite-brand.ts",
	"vitest.config.ts",
	"capacitor.config.ts",
];
const SKIPPED_DIRS = new Set([
	"node_modules",
	"dist",
	"generated",
	"test-results",
	"playwright-report",
	// The rules themselves (scripts/check-brand/) spell the banned name.
	"check-brand",
]);
const SOURCE = /\.(ts|tsx|js|mjs|cjs|json|html|css)$/;

async function listSourceFiles(dir: string): Promise<string[]> {
	const entries = await readdir(dir, { withFileTypes: true });
	const nested = await Promise.all(
		entries.map(async (entry) => {
			if (SKIPPED_DIRS.has(entry.name) || entry.name.startsWith(".")) {
				return [];
			}
			const full = path.join(dir, entry.name);
			if (entry.isDirectory()) return listSourceFiles(full);
			return SOURCE.test(entry.name) ? [full] : [];
		}),
	);
	return nested.flat();
}

async function readAppName(): Promise<string> {
	const config: unknown = JSON.parse(
		await readFile(path.join(ROOT, "brand.config.json"), "utf8"),
	);
	const appName =
		typeof config === "object" && config !== null
			? Reflect.get(config, "appName")
			: undefined;
	if (typeof appName !== "string" || appName === "") {
		throw new Error("brand.config.json has no appName");
	}
	return appName;
}

async function main(): Promise<number> {
	const appName = await readAppName();
	const listed = await Promise.all(
		SCANNED_DIRS.map((dir) => listSourceFiles(path.join(ROOT, dir))),
	);
	const files = [
		...listed.flat(),
		...SCANNED_FILES.map((file) => path.join(ROOT, file)),
	].sort();
	const problems: BrandProblem[] = (
		await Promise.all(
			files.map(async (file) =>
				findBrandProblems(
					await readFile(file, "utf8"),
					path.relative(ROOT, file).split(path.sep).join("/"),
					appName,
				),
			),
		)
	).flat();
	if (problems.length > 0) {
		console.error(`✗ ${problems.length} hard-coded product name(s) in web:`);
		for (const p of problems) {
			console.error(`  ${p.file}:${p.line}  ${p.message}`);
		}
		return 1;
	}
	console.log(`✓ No hard-coded product name in web (${files.length} files)`);
	return 0;
}

main().then(
	(code) => {
		process.exitCode = code;
	},
	(error: unknown) => {
		console.error("check:brand crashed:", error);
		process.exitCode = 1;
	},
);
