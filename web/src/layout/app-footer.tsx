/* Copied from localman-next apps/web/src/components/layout/app-footer.tsx.
   Scribe changes: the hoandat account links instead of the landing, Docs
   and feedback form (Scribe has none); the theme switch moved here from
   the top bar; no language switcher (one language); clears the iPhone
   home indicator (safe-area-inset-bottom). */
import { IconExternalLink } from "@tabler/icons-react";
import { ThemeToggle } from "../components/theme-toggle";
import { useT } from "../i18n/i18n-context";
import { IDP_ACCOUNT_URL, IDP_APPS_URL } from "../lib/idp";

const FOOTER_LINK =
	"inline-flex min-h-11 cursor-pointer items-center gap-1 rounded-sm font-mono font-semibold text-primary underline-offset-4 hover:underline";

/** Page footer: links to the hoandat account pages and the theme switch. */
export function AppFooter() {
	const t = useT();
	const links = [
		{
			id: "account",
			label: t("layout.idpMenu.account"),
			href: IDP_ACCOUNT_URL,
		},
		{ id: "apps", label: t("layout.idpMenu.apps"), href: IDP_APPS_URL },
	];
	return (
		<footer
			data-id="app-footer"
			className="border-border-subtle border-t pt-4 pb-[calc(var(--space-4)+env(safe-area-inset-bottom))] text-[0.8125rem] text-muted"
		>
			<div className="mx-auto flex w-full max-w-page flex-wrap items-center justify-between gap-3 px-gutter">
				<nav
					aria-label={t("layout.footer.links")}
					className="flex flex-wrap items-center gap-4"
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
							<span className="sr-only">{t("layout.idpMenu.opensIdp")}</span>
							<IconExternalLink size={14} aria-hidden="true" />
						</a>
					))}
				</nav>
				<ThemeToggle data-id="app-footer-theme" />
			</div>
		</footer>
	);
}
