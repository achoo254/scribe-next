import AxeBuilder from "@axe-core/playwright";
import type { Locator, Page } from "@playwright/test";
import { FIXED_NOW } from "./api-stubs";

export const COLOR_SCHEMES = ["light", "dark"] as const;
export type ColorScheme = (typeof COLOR_SCHEMES)[number];

/**
 * The smoke set: the journeys CI runs on every push and pull request
 * (`pnpm run test:e2e:smoke`, Chromium only). Everything else runs nightly
 * and before a web deploy. Tag a test only when no unit test can stand in
 * for it: logic belongs in a unit test next to its code.
 */
export const SMOKE = { tag: "@smoke" };

/** The smoke tag for the light run of a test repeated per color scheme. */
export function smokeIn(scheme: ColorScheme): { tag: string[] } {
	return { tag: scheme === "light" ? [SMOKE.tag] : [] };
}

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

/**
 * Waits for the running CSS transitions and animations to end, so axe reads
 * the colours a page settles on. Reduced motion makes them 1ms, but a
 * transition only moves on at the browser's next rendering update, which
 * headless WebKit on Linux may not run for hundreds of ms: axe then saw a
 * just re-enabled button still at its disabled opacity.
 */
async function animationsFinished(page: Page): Promise<void> {
	await page.evaluate(async () => {
		const finite = document
			.getAnimations()
			.filter((animation) =>
				Number.isFinite(animation.effect?.getComputedTiming().endTime),
			);
		// A cancelled animation rejects `finished`; it is over all the same.
		await Promise.all(
			finite.map((animation) => animation.finished.catch(() => undefined)),
		);
	});
}

export interface A11yFinding {
	id: string;
	impact: string | null;
	targets: string[];
}

/**
 * WCAG 2.0/2.1 A and AA violations of impact "serious" or "critical".
 * `include` limits the scan to one element, e.g. an open menu: Radix marks
 * the rest of the page `aria-hidden` while it traps focus in the menu, which
 * axe reports as `aria-hidden-focus` although nothing outside can be reached.
 */
export async function seriousA11yViolations(
	page: Page,
	include?: string,
): Promise<A11yFinding[]> {
	await animationsFinished(page);
	const builder = new AxeBuilder({ page }).withTags([
		"wcag2a",
		"wcag2aa",
		"wcag21a",
		"wcag21aa",
	]);
	const results = await (include
		? builder.include(include)
		: builder
	).analyze();
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

/** The trigger (`<summary>`) of the shared account menu in the top bar. */
export function accountTrigger(page: Page): Locator {
	return page.getByTestId("topbar-account");
}

/** An item (button or link) of the open account menu by its name. */
export function accountMenuItem(page: Page, name: string | RegExp): Locator {
	const menu = page.getByTestId("account-menu");
	return menu
		.getByRole("button", { name })
		.or(menu.getByRole("link", { name }));
}
