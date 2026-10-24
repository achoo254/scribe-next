/* Copied from localman-next apps/web/src/components/layout/app-header.tsx.
   Scribe changes: below 640px the nav becomes a segmented row under the
   bar; no
   feature flags, workspace switcher, docs, language switcher or
   notification bell; the bar clears the iPhone status bar
   (safe-area-inset-top). */
import {
	IconFileText,
	IconMicrophone,
	IconScissors,
} from "@tabler/icons-react";
import { Link, NavLink } from "react-router";
import type { CurrentUser } from "../auth/auth-api";
import { BrandMark, Wordmark } from "../components/brand/wordmark";
import { buttonClassName } from "../components/ui/button";
import { useT } from "../i18n/i18n-context";
import { isNativeShell } from "../lib/api";
import { cn } from "../lib/cn";
import { AccountControls } from "./account-controls";
import { PATHS } from "./app-paths";
import { EcosystemAppSwitcher } from "./ecosystem-app-switcher";
import { useSessionActivity } from "./session-activity";

/** `NavLink` sets `aria-current="page"` on the page shown (and its children). */
// The current page's pill is opaque (the 10% tint mixed into the page
// background): the bar is translucent, and a coloured button scrolling under
// a see-through pill dropped its text below 4.5:1.
const NAV_ITEM_CLASS = cn(
	buttonClassName({ variant: "ghost", size: "sm" }),
	"no-underline aria-[current=page]:bg-[color-mix(in_srgb,var(--color-primary)_10%,var(--color-bg))] aria-[current=page]:text-primary",
);

export interface AppHeaderProps {
	user: CurrentUser;
}

/**
 * Signed-in top bar: wordmark (home: Ghi âm) and the nav on the left,
 * the switcher to the other apps and the account menu on the right. Sticky, so the nav stays in reach.
 */
export function AppHeader({ user }: AppHeaderProps) {
	const t = useT();
	const session = useSessionActivity();
	const items: ReadonlyArray<{
		id: string;
		to: string;
		label: string;
		Icon: typeof IconMicrophone;
	}> = [
		{
			id: "record",
			to: PATHS.record,
			label: t("layout.appHeader.record"),
			Icon: IconMicrophone,
		},
		{
			id: "split",
			to: PATHS.split,
			label: t("layout.appHeader.split"),
			Icon: IconScissors,
		},
		...(user.canTranscribe
			? [
					{
						id: "transcript",
						to: PATHS.transcripts,
						label: t("layout.appHeader.transcript"),
						Icon: IconFileText,
					},
				]
			: []),
	];

	function navItems(prefix: string, className: string) {
		return items.map(({ id, to, label, Icon }) => (
			<NavLink
				key={id}
				to={to}
				data-id={`${prefix}-${id}`}
				className={cn(NAV_ITEM_CLASS, className)}
			>
				<Icon size={16} aria-hidden="true" />
				{label}
			</NavLink>
		));
	}

	return (
		<header
			data-id="topbar"
			className="sticky top-0 z-30 border-border-subtle border-b bg-bg/82 pt-[env(safe-area-inset-top)] backdrop-blur-md"
		>
			{/* Relative below 640px: the app switcher's panel hangs from this row. */}
			<div className="mx-auto flex min-h-(--topbar-h) w-full max-w-page items-center gap-3 px-gutter max-sm:relative">
				<Link
					to={PATHS.record}
					data-id="topbar-home"
					aria-label={t("layout.appHeader.home")}
					className="flex items-center gap-2 rounded-sm no-underline"
				>
					<BrandMark size={28} />
					<Wordmark />
				</Link>
				<nav
					aria-label={t("layout.appHeader.nav")}
					data-id="topbar-nav"
					className="flex gap-0.5 max-sm:hidden"
				>
					{navItems("topbar-nav", "")}
				</nav>
				<div className="ml-auto flex items-center gap-2">
					<EcosystemAppSwitcher />
					<AccountControls
						user={user}
						activity={session.activity}
						beforeSignOut={session.stopRecordings}
						compact={isNativeShell()}
					/>
				</div>
			</div>
			<div className="px-gutter pb-2 sm:hidden">
				<nav
					aria-label={t("layout.appHeader.nav")}
					data-id="topbar-nav-compact"
					className="flex gap-1 rounded-md border border-border bg-surface p-1"
				>
					{navItems("topbar-nav-compact", "flex-1 px-2")}
				</nav>
			</div>
		</header>
	);
}
