import {
	PublicPage,
	type ShellBrand,
	type ShellLink,
	type ShellLinkComponent,
} from "@achoo254/app-shell";
import type { ReactNode } from "react";
import { Link, useLocation } from "react-router";
import scribeMark from "../../brand/scribe-mark-128.webp";
import { useI18n } from "../i18n/i18n-context";
import { APP_NAME, PRIMARY_TENANT_DOMAIN } from "../lib/brand";
import { cn } from "../lib/cn";
import { LEGACY_THEME_KEY } from "../lib/theme-bootstrap";
import { PATHS } from "./app-paths";
import { MAIN_ID, SkipLink } from "./skip-link";

/** What the shared shell shows of Scribe: name, root domain, AI mark. */
export const SHELL_BRAND: ShellBrand = {
	appName: APP_NAME,
	domain: PRIMARY_TENANT_DOMAIN,
	markSrc: scribeMark,
};

/** Scribe's own paths go through the router, without a reload. */
const RouterLink: ShellLinkComponent = ({ href, ...rest }) => (
	<Link to={href} {...rest} />
);

/**
 * The language switch stores the choice itself (`hd_lang`);
 * `PublicI18nProvider` re-renders the page in it, so nothing is left to do.
 */
function followStoredLocale(): void {}

/**
 * A signed-out page of Scribe: the shared header, footer (language and
 * theme switches, ecosystem and legal links) and frame of every
 * hoandat.com app (@achoo254/app-shell), around Scribe's own body. Must
 * render under `PublicI18nProvider`, which gives it the visitor's language.
 */
export function ScribePublicPage({
	children,
	layout = "flow",
	navLinks,
}: {
	children: ReactNode;
	/** "center" for the sign-in card alone (/login), "flow" otherwise. */
	layout?: "flow" | "center";
	/** Header links before "Đăng nhập", e.g. the landing's sections. */
	navLinks?: readonly ShellLink[];
}) {
	const { locale } = useI18n();
	const { pathname } = useLocation();
	return (
		<>
			<SkipLink />
			<PublicPage
				brand={SHELL_BRAND}
				locale={locale}
				onLocaleChange={followStoredLocale}
				signInHref={PATHS.login}
				signInCurrent={pathname === PATHS.login}
				navLinks={navLinks}
				privacyHref={PATHS.privacy}
				termsHref={PATHS.terms}
				legacyThemeKey={LEGACY_THEME_KEY}
				linkComponent={RouterLink}
				layout={layout}
			>
				{/* The skip link's target: the shell's <main> has no id. */}
				<div
					id={MAIN_ID}
					tabIndex={-1}
					className={cn(
						"w-full focus:outline-none",
						layout === "center" && "grid justify-items-center",
					)}
				>
					{children}
				</div>
			</PublicPage>
		</>
	);
}
