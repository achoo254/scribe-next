/* Copied from localman-next apps/web/src/components/layout/skip-link.tsx.
   Scribe change: `MAIN_CLASS` stacks the page blocks (grid gap) since the
   shell renders several of them, and has no top padding of its own on
   phones under the segmented nav. */
import { useT } from "../i18n/i18n-context";

/** Id of the `<main>` element, target of the skip link. */
export const MAIN_ID = "main";

/**
 * First focusable element of every page: jumps keyboard and screen reader
 * users past the top bar. Hidden until it receives focus.
 */
export function SkipLink() {
	const t = useT();
	return (
		<a
			href={`#${MAIN_ID}`}
			data-id="skip-to-main"
			className="sr-only z-90 rounded-md bg-surface px-4 py-2 font-mono font-semibold text-sm shadow-lg focus:not-sr-only focus:fixed focus:top-2 focus:left-2"
		>
			{t("layout.skipLink")}
		</a>
	);
}

/** Class list of the signed-in layout's `<main>`. */
export const MAIN_CLASS =
	"mx-auto grid w-full max-w-page content-start gap-4 px-gutter pt-6 pb-16 focus:outline-none sm:pt-8";
