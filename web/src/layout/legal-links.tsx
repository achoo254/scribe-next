import { Link } from "react-router";
import { useT } from "../i18n/i18n-context";
import { cn } from "../lib/cn";
import { PATHS } from "./app-paths";

/** An underlined link inside running text. */
export const TEXT_LINK =
	"font-semibold text-primary underline underline-offset-4 hover:no-underline";

/** A quiet link under a card, like the sign-in page's legal row. */
export const QUIET_LINK =
	"inline-flex min-h-8 items-center rounded-sm text-muted underline-offset-4 hover:text-fg hover:underline pointer-coarse:min-h-11";

/** "Quyền riêng tư · Điều khoản": the footer, sign-in page and legal pages. */
export function LegalLinks({
	className,
	linkClassName = TEXT_LINK,
}: {
	className?: string;
	linkClassName?: string;
}) {
	const t = useT();
	return (
		<nav
			aria-label={t("legal.nav.label")}
			className={cn("flex items-center gap-3 sm:gap-4", className)}
		>
			<Link
				to={PATHS.privacy}
				data-id="legal-link-privacy"
				className={linkClassName}
			>
				{t("legal.nav.privacy")}
			</Link>
			<Link
				to={PATHS.terms}
				data-id="legal-link-terms"
				className={linkClassName}
			>
				{t("legal.nav.terms")}
			</Link>
		</nav>
	);
}
