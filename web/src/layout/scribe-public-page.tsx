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
import { followStoredLocale } from "../i18n/i18n-provider";
import { LOCALES } from "../i18n/locales";
import { APP_NAME, PRIMARY_TENANT_DOMAIN } from "../lib/brand";
import { LEGACY_THEME_KEY } from "../lib/theme-bootstrap";
import { PATHS } from "./app-paths";

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
 * A signed-out page of Scribe: the shared header, footer (language and
 * theme switches, ecosystem and legal links) and frame of every
 * hoandat.com app (@achoo254/app-shell), around Scribe's own body: the
 * shell also gives it the skip link and `<main id="main">`. In the
 * language of `I18nProvider`.
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
		<PublicPage
			brand={SHELL_BRAND}
			locale={locale}
			locales={LOCALES}
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
			{children}
		</PublicPage>
	);
}
