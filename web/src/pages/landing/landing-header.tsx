import { IconLogin2 } from "@tabler/icons-react";
import { Link } from "react-router";
import { BrandMark, Wordmark } from "../../components/brand/wordmark";
import { buttonClassName } from "../../components/ui/button";
import { useT } from "../../i18n/i18n-context";
import { cn } from "../../lib/cn";

/** In-page anchors of the landing sections. */
export const LANDING_ANCHORS = { features: "features", how: "how" } as const;

const NAV_LINK = cn(
	buttonClassName({ variant: "ghost", size: "sm" }),
	"no-underline",
);

/**
 * Landing top bar: the mark and name, the section anchors (from 640px), and
 * "Đăng nhập". Sticky like the app's bar; the anchors land below it
 * (`scroll-padding-top` in global.css).
 */
export function LandingHeader() {
	const t = useT();
	return (
		<header
			data-id="landing-header"
			className="sticky top-0 z-30 border-border-subtle border-b bg-bg/82 pt-[env(safe-area-inset-top)] backdrop-blur-md"
		>
			<div className="mx-auto flex min-h-(--topbar-h) w-full max-w-page items-center gap-3 px-gutter">
				<Link
					to="/"
					data-id="landing-home"
					className="flex min-h-11 items-center gap-2 rounded-sm no-underline"
				>
					<BrandMark />
					<Wordmark />
				</Link>
				<nav
					aria-label={t("landing.nav.label")}
					className="ml-4 flex gap-1 max-sm:hidden"
				>
					<a
						href={`#${LANDING_ANCHORS.features}`}
						data-id="landing-nav-features"
						className={NAV_LINK}
					>
						{t("landing.nav.features")}
					</a>
					<a
						href={`#${LANDING_ANCHORS.how}`}
						data-id="landing-nav-how"
						className={NAV_LINK}
					>
						{t("landing.nav.how")}
					</a>
				</nav>
				<Link
					to="/login"
					data-id="landing-sign-in"
					className={cn(
						buttonClassName({ variant: "secondary", size: "md" }),
						"ml-auto min-h-11 no-underline",
					)}
				>
					<IconLogin2 size={18} aria-hidden="true" />
					{t("landing.nav.signIn")}
				</Link>
			</div>
		</header>
	);
}
