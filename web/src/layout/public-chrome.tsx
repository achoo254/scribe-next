import { Link, useLocation } from "react-router";
import { BrandMark, Wordmark } from "../components/brand/wordmark";
import { useI18n, useT } from "../i18n/i18n-context";
import { LOCALES, LOCALE_NAMES } from "../i18n/locales";
import { ECOSYSTEM_HOME_URL } from "../lib/brand";
import { cn } from "../lib/cn";
import { PATHS } from "./app-paths";

/*
 * Top bar and footer of the public pages (landing, sign-in, privacy,
 * terms), built like the account app at auth.hoandat.com, the base layout
 * of every hoandat.com app: auth-next src/ui/layout.ts and the .topbar,
 * .idp-brand, .topbar-link, .page-foot, .lang-switch and .foot-link rules
 * of its static/css/app.css. Phones (< 480px) and touch screens get 44px
 * targets on every header and footer link, as there.
 */

/** 44px targets on touch screens and phones (auth: pointer coarse or ≤ 479px). */
const TAP_TARGET =
	"pointer-coarse:inline-flex pointer-coarse:min-h-11 pointer-coarse:items-center max-[479px]:inline-flex max-[479px]:min-h-11 max-[479px]:items-center";

/** `.topbar-link`: small mono label, a tinted pill for the current page. */
export const TOPBAR_LINK = cn(
	"whitespace-nowrap rounded-sm px-3 py-2 font-mono font-semibold text-[0.8125rem] text-muted no-underline transition-colors",
	"hover:bg-fg/6 hover:text-fg aria-[current=page]:bg-primary/10 aria-[current=page]:text-primary",
	"max-[479px]:px-2 max-[379px]:px-1",
	TAP_TARGET,
);

/** The call to sign in: always the tinted pill, so it reads as the action. */
const SIGN_IN_LINK = cn(
	TOPBAR_LINK,
	"bg-primary/10 text-primary hover:bg-primary/16 hover:text-primary",
);

export interface PublicNavLink {
	/** In-page anchor (`#features`) or a route. */
	href: string;
	label: string;
	"data-id": string;
}

/**
 * Sticky top bar: the AI mark (24px) and the app name on the left, then
 * the page's own links (hidden under 380px when `hideLinksOnPhones`) and
 * "Đăng nhập", marked current on /login.
 */
export function PublicHeader({
	links = [],
	hideLinksOnPhones = false,
}: {
	links?: readonly PublicNavLink[];
	hideLinksOnPhones?: boolean;
}) {
	const t = useT();
	const { pathname } = useLocation();
	return (
		<header
			data-id="public-header"
			className="sticky top-0 z-30 border-border-subtle border-b bg-bg/82 pt-[env(safe-area-inset-top)] backdrop-blur-md"
		>
			<div className="mx-auto flex min-h-(--topbar-h) w-full max-w-page items-center justify-between gap-3 px-gutter max-[359px]:gap-2">
				<Link
					to="/"
					data-id="public-home"
					className="inline-flex min-h-11 items-center gap-2 rounded-sm no-underline"
				>
					<BrandMark size={24} />
					<Wordmark />
				</Link>
				<nav
					aria-label={t("layout.publicChrome.nav")}
					className="flex items-center gap-1"
				>
					{links.map(({ href, label, "data-id": dataId }) => (
						<a
							key={dataId}
							href={href}
							data-id={dataId}
							className={cn(
								TOPBAR_LINK,
								hideLinksOnPhones && "max-[379px]:hidden",
							)}
						>
							{label}
						</a>
					))}
					<Link
						to="/login"
						data-id="public-sign-in"
						aria-current={pathname === "/login" ? "page" : undefined}
						className={SIGN_IN_LINK}
					>
						{t("layout.publicChrome.signIn")}
					</Link>
				</nav>
			</div>
		</header>
	);
}

const FOOT_LINK = cn(
	"rounded-sm text-[0.8125rem] text-muted no-underline underline-offset-2 hover:text-fg hover:underline",
	TAP_TARGET,
);

const LANG_OPTION = cn(
	"rounded-full border border-transparent px-3 py-1 font-mono font-semibold text-[0.8125rem] text-muted no-underline hover:text-fg",
	"aria-[current=true]:border-[color-mix(in_srgb,var(--c-primary)_30%,var(--c-surface))] aria-[current=true]:bg-[color-mix(in_srgb,var(--c-primary)_8%,var(--c-surface))] aria-[current=true]:text-primary",
	TAP_TARGET,
);

/**
 * Footer: the language pill (one link per language, the current one
 * marked), then "Một app của hoandat.com", the privacy policy and terms.
 */
export function PublicFooter() {
	const t = useT();
	const { locale } = useI18n();
	const { pathname } = useLocation();
	return (
		<footer
			data-id="public-footer"
			className="mx-auto flex w-full max-w-page flex-col items-center gap-3 px-gutter pb-[calc(var(--space-7)+env(safe-area-inset-bottom))]"
		>
			<nav
				aria-label={t("layout.publicChrome.language")}
				data-id="lang-switch"
				className="inline-flex flex-wrap justify-center gap-0.5 rounded-full border border-border bg-surface p-0.5"
			>
				{LOCALES.map((option) => (
					<Link
						key={option}
						to={pathname}
						lang={option}
						hrefLang={option}
						aria-current={option === locale ? "true" : undefined}
						data-id={`lang-${option}`}
						className={LANG_OPTION}
					>
						{LOCALE_NAMES[option]}
					</Link>
				))}
			</nav>
			<nav aria-label={t("layout.publicChrome.footLinks")}>
				<p className="m-0 flex flex-wrap justify-center gap-x-4">
					<a
						href={ECOSYSTEM_HOME_URL}
						data-id="foot-home"
						className={FOOT_LINK}
					>
						{t("layout.publicChrome.ecosystem")}
					</a>
					<Link to={PATHS.privacy} data-id="foot-privacy" className={FOOT_LINK}>
						{t("legal.nav.privacy")}
					</Link>
					<Link to={PATHS.terms} data-id="foot-terms" className={FOOT_LINK}>
						{t("legal.nav.terms")}
					</Link>
				</p>
			</nav>
		</footer>
	);
}
