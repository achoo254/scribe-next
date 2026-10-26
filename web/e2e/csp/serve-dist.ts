import { readFile, stat } from "node:fs/promises";
/**
 * Serves the production build (`web/dist`) the way nginx does
 * (deploy/nginx-scribe-locations.inc): static files, SPA fallback to index.html,
 * and the security headers of every response. `/api/` never reaches it:
 * the specs stub the API in the browser.
 *
 * The CSP gets a `report-uri` so that every violation, in any spec, is
 * POSTed back here; `GET /__csp-reports` hands them to the global
 * teardown (csp-teardown.ts), which fails the run when there is one.
 */
import { createServer } from "node:http";
import type { IncomingMessage, ServerResponse } from "node:http";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { E2E_PORT } from "../support/e2e-origin";
import { CSP_REPORTS_PATH } from "./csp-reports";
import { loadSecurityHeaders } from "./security-headers";

const DIST = path.resolve(
	path.dirname(fileURLToPath(import.meta.url)),
	"../../dist",
);

const MIME: Record<string, string> = {
	".html": "text/html; charset=utf-8",
	".js": "text/javascript; charset=utf-8",
	".css": "text/css; charset=utf-8",
	".json": "application/json",
	".webmanifest": "application/manifest+json",
	".svg": "image/svg+xml",
	".png": "image/png",
	".webp": "image/webp",
	".ico": "image/x-icon",
	".woff2": "font/woff2",
};

const MAX_REPORT_BYTES = 64 * 1024;

const securityHeaders = loadSecurityHeaders();
const headers: Record<string, string> = {
	...securityHeaders,
	"Content-Security-Policy": `${securityHeaders["Content-Security-Policy"]}; report-uri ${CSP_REPORTS_PATH}`,
};
const reports: unknown[] = [];

function send(
	res: ServerResponse,
	status: number,
	type: string,
	body: string | Buffer,
): void {
	res.writeHead(status, { ...headers, "Content-Type": type });
	res.end(body);
}

function readBody(req: IncomingMessage): Promise<string> {
	return new Promise((resolve, reject) => {
		let size = 0;
		const chunks: Buffer[] = [];
		req.on("data", (chunk: Buffer) => {
			size += chunk.length;
			if (size > MAX_REPORT_BYTES) {
				reject(new Error("CSP report too large"));
				req.destroy();
				return;
			}
			chunks.push(chunk);
		});
		req.on("end", () => resolve(Buffer.concat(chunks).toString("utf8")));
		req.on("error", reject);
	});
}

/** File under dist for a URL path, or null when it would leave dist. */
function distFile(pathname: string): string | null {
	const file = path.resolve(DIST, `.${decodeURIComponent(pathname)}`);
	return file === DIST || file.startsWith(`${DIST}${path.sep}`) ? file : null;
}

async function isFile(file: string): Promise<boolean> {
	try {
		return (await stat(file)).isFile();
	} catch {
		return false;
	}
}

async function handle(req: IncomingMessage, res: ServerResponse) {
	const url = new URL(req.url ?? "/", "http://localhost");

	if (url.pathname === CSP_REPORTS_PATH) {
		if (req.method === "POST") {
			const body = await readBody(req);
			try {
				reports.push(JSON.parse(body));
			} catch {
				reports.push({ unparsed: body });
			}
			console.error(`CSP violation: ${body}`);
			res.writeHead(204).end();
			return;
		}
		send(res, 200, MIME[".json"] ?? "", JSON.stringify(reports));
		return;
	}

	if (req.method !== "GET" && req.method !== "HEAD") {
		send(res, 405, "text/plain", "method not allowed");
		return;
	}

	const file = distFile(url.pathname);
	if (file === null) {
		send(res, 400, "text/plain", "bad path");
		return;
	}
	if (await isFile(file)) {
		const type = MIME[path.extname(file)] ?? "application/octet-stream";
		send(res, 200, type, await readFile(file));
		return;
	}
	// nginx: /assets/ answers 404, everything else falls back to the SPA.
	if (url.pathname.startsWith("/assets/")) {
		send(res, 404, "text/plain", "not found");
		return;
	}
	send(
		res,
		200,
		MIME[".html"] ?? "",
		await readFile(path.join(DIST, "index.html")),
	);
}

const server = createServer((req, res) => {
	handle(req, res).catch((error: unknown) => {
		console.error("serve-dist:", error);
		if (!res.headersSent) send(res, 500, "text/plain", "server error");
		else res.destroy();
	});
});

if (!(await isFile(path.join(DIST, "index.html")))) {
	console.error(`✗ No build at ${DIST}: run pnpm run build first`);
	process.exit(1);
}

server.listen(E2E_PORT, "127.0.0.1", () => {
	console.log(`dist with production headers on http://127.0.0.1:${E2E_PORT}`);
});
