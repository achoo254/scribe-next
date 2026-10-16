/* Copied from localman-next apps/web/src/components/layout/app-header.tsx.
   Scribe changes: the nav switches the shell's in-page views (no URL per
   page yet) and below 640px becomes a segmented row under the bar; no
   feature flags, workspace switcher, docs, language switcher or
   notification bell; the bar clears the iPhone status bar
   (safe-area-inset-top). */
import {
	IconFileText,
	IconMicrophone,
	IconScissors,
} from "@tabler/icons-react";
import { Link } from "react-router";
import type { CurrentUser } from "../auth/auth-api";
import { BrandMark, Wordmark } from "../components/brand/wordmark";
import { buttonClassName } from "../components/ui/button";
import { useT } from "../i18n/i18n-context";
import { isNativeShell } from "../lib/api";
import { cn } from "../lib/cn";
import { AccountControls } from "./account-controls";
import type { SessionActivity } from "./session-activity";

export type AppTab = "record" | "split" | "transcript";

const NAV_ITEM_CLASS = cn(
	buttonClassName({ variant: "ghost", size: "sm" }),
	"aria-[current=page]:bg-primary/10 aria-[current=page]:text-primary",
);

export interface AppHeaderProps {
	user: CurrentUser;
	tab: AppTab;
	onTabChange: (tab: AppTab) => void;
	activity: SessionActivity;
	beforeSignOut: () => Promise<void>;
}

/**
 * Signed-in top bar: wordmark (home: Ghi âm) and the nav on the left,
 * the account menu on the right. Sticky, so the nav stays in reach.
 */
export function AppHeader({
	user,
	tab,
	onTabChange,
	activity,
	beforeSignOut,
}: AppHeaderProps) {
	const t = useT();
	const items: ReadonlyArray<{
		value: AppTab;
		label: string;
		Icon: typeof IconMicrophone;
	}> = [
		{
			value: "record",
			label: t("layout.appHeader.record"),
			Icon: IconMicrophone,
		},
		{ value: "split", label: t("layout.appHeader.split"), Icon: IconScissors },
		...(user.canTranscribe
			? [
					{
						value: "transcript" as const,
						label: t("layout.appHeader.transcript"),
						Icon: IconFileText,
					},
				]
			: []),
	];

	function navItems(prefix: string, className: string) {
		return items.map(({ value, label, Icon }) => (
			<button
				key={value}
				type="button"
				data-id={`${prefix}-${value}`}
				aria-current={tab === value ? "page" : undefined}
				onClick={() => onTabChange(value)}
				className={cn(NAV_ITEM_CLASS, className)}
			>
				<Icon size={16} aria-hidden="true" />
				{label}
			</button>
		));
	}

	return (
		<header
			data-id="topbar"
			className="sticky top-0 z-30 border-border-subtle border-b bg-bg/82 pt-[env(safe-area-inset-top)] backdrop-blur-md"
		>
			<div className="mx-auto flex min-h-(--topbar-h) w-full max-w-page items-center gap-3 px-gutter">
				<Link
					to="/"
					data-id="topbar-home"
					aria-label={t("layout.appHeader.home")}
					onClick={() => onTabChange("record")}
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
					<AccountControls
						user={user}
						activity={activity}
						beforeSignOut={beforeSignOut}
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
