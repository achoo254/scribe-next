import { ThemeCycleButton, ThemeToggle } from "../../components/theme-toggle";
import { useT } from "../../i18n/i18n-context";
import { renderRich } from "../../i18n/rich-text";
import { LegalLinks } from "../../layout/legal-links";
import { ECOSYSTEM_HOME_URL } from "../../lib/brand";

const FOOTER_LINK =
	"inline-flex min-h-11 items-center rounded-sm font-semibold text-primary underline-offset-4 hover:underline";

/**
 * Landing footer: "Một app của hoandat.com" (the ecosystem's home), the
 * privacy policy and terms, and the theme switch (one button on phones,
 * like the app footer).
 */
export function LandingFooter() {
	const t = useT();
	return (
		<footer
			data-id="landing-footer"
			className="border-border-subtle border-t py-4 pb-[calc(var(--space-4)+env(safe-area-inset-bottom))] text-muted text-sm"
		>
			<div className="mx-auto flex w-full max-w-page flex-wrap items-center justify-between gap-x-6 gap-y-2 px-gutter">
				<div className="flex min-w-0 flex-wrap items-center gap-x-6 gap-y-1">
					<p className="m-0">
						{renderRich(t("landing.footer.madeBy"), {
							home: (chunk) => (
								<a
									href={ECOSYSTEM_HOME_URL}
									data-id="landing-footer-home"
									className={FOOTER_LINK}
								>
									{chunk}
								</a>
							),
						})}
					</p>
					<LegalLinks className="gap-5 sm:gap-5" linkClassName={FOOTER_LINK} />
				</div>
				<ThemeCycleButton
					data-id="landing-footer-theme-cycle"
					className="sm:hidden"
				/>
				<div className="hidden sm:block">
					<ThemeToggle data-id="landing-footer-theme" />
				</div>
			</div>
		</footer>
	);
}
