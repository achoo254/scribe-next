import AxeBuilder from "@axe-core/playwright";
import type { Page } from "@playwright/test";
import { FIXED_NOW } from "./api-stubs";

export const COLOR_SCHEMES = ["light", "dark"] as const;
export type ColorScheme = (typeof COLOR_SCHEMES)[number];

/**
 * Freezes `Date` on the fixtures' day and follows the OS theme (no saved
 * choice), so screenshots do not change with the day or the machine.
 */
export async function preparePage(
	page: Page,
	colorScheme: ColorScheme,
): Promise<void> {
	await page.clock.setFixedTime(FIXED_NOW);
	await page.emulateMedia({ colorScheme, reducedMotion: "reduce" });
}

/** Waits for the self-hosted fonts, so text is laid out before a screenshot. */
export async function fontsReady(page: Page): Promise<void> {
	await page.evaluate(async () => {
		await document.fonts.ready;
	});
}

export interface A11yFinding {
	id: string;
	impact: string | null;
	targets: string[];
}

/** WCAG 2.0/2.1 A and AA violations of impact "serious" or "critical". */
export async function seriousA11yViolations(
	page: Page,
): Promise<A11yFinding[]> {
	const results = await new AxeBuilder({ page })
		.withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"])
		.analyze();
	return results.violations
		.filter(
			(violation) =>
				violation.impact === "serious" || violation.impact === "critical",
		)
		.map((violation) => ({
			id: violation.id,
			impact: violation.impact ?? null,
			targets: violation.nodes.slice(0, 3).map((node) => node.target.join(" ")),
		}));
}
