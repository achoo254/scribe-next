/**
 * `npm run check:tokens` — design-token guard for OneScribe web (copied
 * from localman-next, which owns the same design system).
 *
 * 1. Literal scan: `src/` and `index.html` must not contain colour literals
 *    (hex, rgb()/hsl()/…) or raw px spacing; those live only in
 *    src/styles/tokens.css. Allowlist and ignore marker are documented in
 *    scripts/check-tokens/scan-literals.ts.
 * 2. Token consistency: the OS-preference dark block and the forced dark
 *    block in tokens.css must stay identical.
 * 3. Contrast: main text/background pairs meet WCAG 4.5:1 (UI parts 3:1) in
 *    light and dark; APCA Lc is printed as an advisory value.
 *
 * Exits 1 on any failure, printing `file:line` for literal violations.
 */
import { readFile, readdir } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
	APCA_BODY_TEXT_LC,
	type ContrastResult,
	evaluatePairs,
} from "./check-tokens/contrast.js";
import { diffDarkBlocks, resolveThemes } from "./check-tokens/parse-tokens.js";
import {
	type LiteralViolation,
	findLiteralViolations,
} from "./check-tokens/scan-literals.js";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const TOKENS_FILE = path.join(ROOT, "src/styles/tokens.css");
const WEB_SRC = path.join(ROOT, "src");
const EXTRA_FILES = [path.join(ROOT, "index.html")];
const SCANNED_EXTENSIONS = new Set([".ts", ".tsx", ".css"]);

async function listSourceFiles(dir: string): Promise<string[]> {
	const entries = await readdir(dir, { withFileTypes: true });
	const nested = await Promise.all(
		entries.map(async (entry) => {
			const full = path.join(dir, entry.name);
			if (entry.isDirectory()) return listSourceFiles(full);
			return SCANNED_EXTENSIONS.has(path.extname(entry.name)) ? [full] : [];
		}),
	);
	return nested.flat();
}

async function scanLiterals(): Promise<LiteralViolation[]> {
	const sources = await listSourceFiles(WEB_SRC);
	const files = [
		...sources.filter((file) => file !== TOKENS_FILE),
		...EXTRA_FILES,
	].sort();
	const results = await Promise.all(
		files.map(async (file) =>
			findLiteralViolations(
				await readFile(file, "utf8"),
				path.relative(ROOT, file),
			),
		),
	);
	return results.flat();
}

function formatResult(result: ContrastResult): string {
	const status = result.pass ? "ok  " : "FAIL";
	const ratio = result.ratio.toFixed(2).padStart(5);
	const lc = result.apcaLc.toFixed(1).padStart(6);
	return `  ${status} ${result.theme.padEnd(5)} ${ratio}:1  Lc ${lc}  ${result.fg} on ${result.bg} (${result.kind})`;
}

async function main(): Promise<number> {
	let failed = false;

	const violations = await scanLiterals();
	if (violations.length > 0) {
		failed = true;
		console.error(
			`✗ ${violations.length} design-token bypass(es) in web (use a token from src/styles/tokens.css):`,
		);
		for (const v of violations) {
			console.error(`  ${v.file}:${v.line}  [${v.rule}]  ${v.match}`);
		}
	} else {
		console.log("✓ No colour literals or raw px spacing in web");
	}

	const themes = resolveThemes(await readFile(TOKENS_FILE, "utf8"));
	const drift = diffDarkBlocks(themes);
	if (drift.length > 0) {
		failed = true;
		console.error("✗ Dark token blocks drifted apart in tokens.css:");
		for (const line of drift) console.error(`  ${line}`);
	} else {
		console.log("✓ Dark blocks (media query and data-theme) are identical");
	}

	const results = [
		...evaluatePairs("light", themes.light),
		...evaluatePairs("dark", themes.dark),
	];
	const failures = results.filter((r) => !r.pass);
	const lowApca = results.filter(
		(r) =>
			r.pass && r.kind === "text" && Math.abs(r.apcaLc) < APCA_BODY_TEXT_LC,
	);
	if (failures.length > 0) {
		failed = true;
		console.error(`✗ ${failures.length} contrast pair(s) below WCAG minimum:`);
		for (const r of failures) console.error(formatResult(r));
	} else {
		console.log(
			`✓ ${results.length} contrast pairs meet WCAG (text 4.5:1, UI 3:1) in light and dark`,
		);
	}
	if (lowApca.length > 0) {
		console.warn(
			`! Advisory: ${lowApca.length} text pair(s) under APCA Lc ${APCA_BODY_TEXT_LC} (fine for labels/bold text, avoid for long body copy):`,
		);
		for (const r of lowApca) console.warn(formatResult(r));
	}

	return failed ? 1 : 0;
}

main().then(
	(code) => {
		process.exitCode = code;
	},
	(error: unknown) => {
		console.error("check:tokens crashed:", error);
		process.exitCode = 1;
	},
);
