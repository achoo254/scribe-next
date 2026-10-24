/* Copied from localman-next apps/web/src/components/layout/app-footer.tsx.
   Scribe changes: the IdP account links instead of the landing, Docs
   and feedback form (Scribe has none); the theme switch moved here from
   the top bar; no language switcher (one language); clears the iPhone
   home indicator (safe-area-inset-bottom); links to the privacy policy and
   terms (wrapping under the account links on phones). */
import { IconExternalLink } from "@tabler/icons-react";
import { ThemeCycleButton, ThemeToggle } from "../components/theme-toggle";
import { useT } from "../i18n/i18n-context";
import { idpAccountUrlOf, idpAppsUrlOf } from "../lib/idp";
import { useTenantInfo } from "../lib/tenant-info";
import { LegalLinks } from "./legal-links";

const FOOTER_LINK =
	"inline-flex min-h-11 cursor-pointer items-center gap-1 whitespace-nowrap rounded-sm font-mono font-semibold text-primary underline-offset-4 hover:underline";

/** Page footer: IdP account pages, privacy and terms, the theme switch. */
export function AppFooter() {
	const t = useT();
	const tenant = useTenantInfo();
	const idpName = tenant.displayName;
	const links = [
		{
			id: "account",
			label: t("layout.idpMenu.account", { idpName }),
			href: idpAccountUrlOf(tenant),
		},
		{ id: "apps", label: t("layout.idpMenu.apps"), href: idpAppsUrlOf(tenant) },
	];
	return (
		<footer
			data-id="app-footer"
			className="border-border-subtle border-t pt-1 pb-[calc(var(--space-1)+env(safe-area-inset-bottom))] text-[0.8125rem] text-muted sm:pt-4 sm:pb-[calc(var(--space-4)+env(safe-area-inset-bottom))]"
		>
			<div className="mx-auto flex w-full max-w-page items-center justify-between gap-3 px-gutter">
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
					<LegalLinks linkClassName={FOOTER_LINK} />
				</div>
				{/* Phones get one button so the theme switch stays on the first row. */}
				<ThemeCycleButton
					data-id="app-footer-theme-cycle"
					className="sm:hidden"
				/>
				<div className="hidden sm:block">
					<ThemeToggle data-id="app-footer-theme" />
				</div>
			</div>
		</footer>
	);
}
