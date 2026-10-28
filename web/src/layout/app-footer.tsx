/* Copied from localman-next apps/web/src/components/layout/app-footer.tsx.
   Scribe changes: the IdP account links instead of the landing, Docs
   and feedback form (Scribe has none); the theme switch moved here from
   the top bar (it stores the shared hoandat.com choice, lib/theme.ts) and
   sits with the shared language switch (@achoo254/app-shell), as in the
   signed-out footer; clears the iPhone home indicator
   (safe-area-inset-bottom); links to the privacy policy and terms. */
import { LanguageSwitch } from "@achoo254/app-shell";
import { IconExternalLink } from "@tabler/icons-react";
import { ThemeCycleButton, ThemeToggle } from "../components/theme-toggle";
import { useI18n } from "../i18n/i18n-context";
import { followStoredLocale } from "../i18n/i18n-provider";
import { LOCALES } from "../i18n/locales";
import { ECOSYSTEM_HOME_URL, PRIMARY_TENANT_DOMAIN } from "../lib/brand";
import { IDP_ACCOUNT_URL, IDP_APPS_URL } from "../lib/idp";
import { useIdpName } from "../lib/idp-name";
import { LegalLinks } from "./legal-links";

const FOOTER_LINK =
	"inline-flex min-h-11 cursor-pointer items-center gap-1 whitespace-nowrap rounded-sm font-mono font-semibold text-primary underline-offset-4 hover:underline";

/**
 * Page footer: the language and theme switches together (their own row on
 * phones, on the right from `sm`), then the IdP account pages, "Một app
 * của hoandat.com", privacy and terms.
 */
export function AppFooter() {
	const { locale, t } = useI18n();
	const idpName = useIdpName();
	const links = [
		{
			id: "account",
			label: t("layout.idpMenu.account", { idpName }),
			href: IDP_ACCOUNT_URL,
		},
		{ id: "apps", label: t("layout.idpMenu.apps"), href: IDP_APPS_URL },
	];
	return (
		<footer
			data-id="app-footer"
			className="border-border-subtle border-t pt-1 pb-[calc(var(--space-1)+env(safe-area-inset-bottom))] text-[0.8125rem] text-muted sm:pt-4 sm:pb-[calc(var(--space-4)+env(safe-area-inset-bottom))]"
		>
			<div className="mx-auto flex w-full max-w-page flex-col gap-1 px-gutter sm:flex-row-reverse sm:items-center sm:justify-between sm:gap-3">
				<div
					data-id="app-footer-switches"
					className="flex flex-wrap items-center gap-2 pt-2 sm:pt-0"
				>
					<LanguageSwitch
						locale={locale}
						locales={LOCALES}
						domain={PRIMARY_TENANT_DOMAIN}
						onLocaleChange={followStoredLocale}
					/>
					{/* Phones get one button so both switches fit on one row. */}
					<ThemeCycleButton
						data-id="app-footer-theme-cycle"
						className="sm:hidden"
					/>
					<div className="hidden sm:block">
						<ThemeToggle data-id="app-footer-theme" />
					</div>
				</div>
				<div className="flex min-w-0 flex-wrap items-center gap-x-3 sm:gap-x-4">
					<nav
						aria-label={t("layout.footer.links")}
						className="flex min-w-0 items-center gap-3 sm:gap-4"
					>
						{links.map(({ id, label, href }) => (
							// In the iOS app Capacitor opens target=_blank links in the
							// system browser.
							<a
								key={id}
								href={href}
								target="_blank"
								rel="noopener noreferrer"
								data-id={`app-footer-${id}`}
								className={FOOTER_LINK}
							>
								{label}
								<span className="sr-only">
									{t("layout.idpMenu.opensIdp", { idpName })}
								</span>
								<IconExternalLink size={14} aria-hidden="true" />
							</a>
						))}
					</nav>
					<a
						href={ECOSYSTEM_HOME_URL}
						data-id="app-footer-home"
						className={FOOTER_LINK}
					>
						{t("layout.footer.ecosystem")}
					</a>
					<LegalLinks linkClassName={FOOTER_LINK} />
				</div>
			</div>
		</footer>
	);
}
