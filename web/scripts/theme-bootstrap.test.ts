// @vitest-environment node
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { THEME_BOOTSTRAP_SCRIPT } from "../src/lib/theme-bootstrap.js";

/*
 * The inline theme script of index.html (vite.config.ts puts in the shared
 * script of @achoo254/app-shell) runs only if the CSP names its hash. A new
 * package version or option changes the text, so this fails first and
 * prints the hash to put in the nginx include.
 */
const SECURITY_HEADERS = new URL(
	"../../deploy/nginx-scribe-security-headers.inc",
	import.meta.url,
);

describe("theme bootstrap script", () => {
	it("is allowed by the CSP hash", () => {
		const hash = createHash("sha256")
			.update(THEME_BOOTSTRAP_SCRIPT)
			.digest("base64");
		const headers = readFileSync(SECURITY_HEADERS, "utf8");
		expect(headers, `script-src needs 'sha256-${hash}'`).toContain(
			`'sha256-${hash}'`,
		);
	});

	it("keeps the visitor's theme from before the shared choice", () => {
		expect(THEME_BOOTSTRAP_SCRIPT).toContain('"scribe-theme"');
		expect(THEME_BOOTSTRAP_SCRIPT).toContain("hd_theme");
	});

	it("has a place in index.html", () => {
		const html = readFileSync(
			new URL("../index.html", import.meta.url),
			"utf8",
		);
		expect(html.split("<!--theme-bootstrap-->")).toHaveLength(2);
		expect(html).not.toMatch(/<script>(?!\s*<\/script>)/);
	});
});
