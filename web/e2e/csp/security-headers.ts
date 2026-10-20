/**
 * The production security headers, read from the nginx include that is
 * deployed (deploy/nginx-scribe-security-headers.inc), so the CSP run
 * tests exactly what nginx sends instead of a copy that can drift.
 */
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

export const SECURITY_HEADERS_FILE = path.resolve(
	path.dirname(fileURLToPath(import.meta.url)),
	"../../../deploy/nginx-scribe-security-headers.inc",
);

/** `add_header Name "value" always;` (the quotes are optional in nginx). */
const ADD_HEADER =
	/^add_header\s+([A-Za-z-]+)\s+(?:"([^"]*)"|(\S+))\s+always;$/;

/**
 * Header name → value of every `add_header` line. Fails on a line it
 * cannot read, so a new directive is never silently left out of the test.
 */
export function parseSecurityHeaders(text: string): Record<string, string> {
	const headers: Record<string, string> = {};
	for (const raw of text.split("\n")) {
		const line = raw.trim();
		if (line === "" || line.startsWith("#")) continue;
		const match = ADD_HEADER.exec(line);
		const name = match?.[1];
		const value = match?.[2] ?? match?.[3];
		if (name === undefined || value === undefined) {
			throw new Error(
				`${SECURITY_HEADERS_FILE}: expected 'add_header Name "value" always;', got: ${line}`,
			);
		}
		headers[name] = value;
	}
	if (!headers["Content-Security-Policy"]) {
		throw new Error(`${SECURITY_HEADERS_FILE} has no Content-Security-Policy`);
	}
	return headers;
}

export function loadSecurityHeaders(): Record<string, string> {
	return parseSecurityHeaders(readFileSync(SECURITY_HEADERS_FILE, "utf8"));
}
