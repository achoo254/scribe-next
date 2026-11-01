import {
	PublicPage,
	type ShellBrand,
	type ShellLink,
	type ShellLinkComponent,
} from "@achoo254/app-shell";
import type { ReactNode } from "react";
import { Link, useLocation } from "react-router";
import scribeMark from "../../brand/scribe-mark-128.webp";
import { WEB_SIGN_UP_PATH } from "../auth/auth-api";
import { useI18n } from "../i18n/i18n-context";
import { followStoredLocale } from "../i18n/i18n-provider";
import { LOCALES } from "../i18n/locales";
import { apiUrl, isNativeShell } from "../lib/api";
import { APP_NAME, PRIMARY_TENANT_DOMAIN } from "../lib/brand";
import { LEGACY_THEME_KEY } from "../lib/theme-bootstrap";
import { PATHS } from "./app-paths";

/** What the shared shell shows of Scribe: name, root domain, AI mark. */
export const SHELL_BRAND: ShellBrand = {
	appName: APP_NAME,
	domain: PRIMARY_TENANT_DOMAIN,
	markSrc: scribeMark,
};

/**
 * Scribe's own pages go through the router, without a reload; a backend
 * route (`/api/…`, e.g. the sign-up) is a plain link the browser follows.
 */
export const RouterLink: ShellLinkComponent = ({ href, ...rest }) =>
	href.startsWith("/api/") ? (
		<a href={href} {...rest} />
	) : (
		<Link to={href} {...rest} />
	);

/**
 * Target of the header pill, labelled "Đăng ký miễn phí" by the shell: on
 * the web the backend sign-up (hoandat-auth opens on "create account"); the
 * apps sign in natively from /login, where the IdP page links to making an
 * account.
 */
function signUpPillHref(native: boolean): string {
	return native ? PATHS.login : apiUrl(WEB_SIGN_UP_PATH);
}

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
	const native = isNativeShell();
	return (
		<PublicPage
			brand={SHELL_BRAND}
			locale={locale}
			locales={LOCALES}
			onLocaleChange={followStoredLocale}
			signInHref={signUpPillHref(native)}
			signInCurrent={native && pathname === PATHS.login}
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
