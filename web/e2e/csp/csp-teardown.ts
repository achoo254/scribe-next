import { E2E_ORIGIN } from "../support/e2e-origin";
import { CSP_REPORTS_PATH } from "./csp-reports";

/**
 * After every spec ran against the production headers: fails the run when
 * the browser reported any CSP violation to serve-dist.ts.
 */
export default async function cspTeardown(): Promise<void> {
	const response = await fetch(`${E2E_ORIGIN}${CSP_REPORTS_PATH}`);
	if (!response.ok) {
		throw new Error(`CSP reports: HTTP ${response.status}`);
	}
	const reports: unknown = await response.json();
	if (!Array.isArray(reports)) {
		throw new Error("CSP reports: expected a JSON array");
	}
	if (reports.length > 0) {
		throw new Error(
			`${reports.length} CSP violation(s) across the specs:\n${reports
				.map((report) => JSON.stringify(report))
				.join("\n")}`,
		);
	}
	console.log("✓ No CSP violation reported across the specs");
}
