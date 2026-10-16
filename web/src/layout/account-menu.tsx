/* Copied from localman-next apps/web/src/components/layout/account-menu.tsx.
   Scribe change: no in-app account, notification, docs, home or admin
   entries (Scribe has none of those pages) and no feature flags; the
   trigger names the email, the only identity the session carries. */
import { IconChevronDown, IconLogout } from "@tabler/icons-react";
import type { CurrentUser } from "../auth/auth-api";
import { buttonClassName } from "../components/ui/button";
import {
	DropdownMenu,
	DropdownMenuContent,
	DropdownMenuItem,
	DropdownMenuLabel,
	DropdownMenuSeparator,
	DropdownMenuTrigger,
} from "../components/ui/dropdown-menu";
import { useT } from "../i18n/i18n-context";
import { cn } from "../lib/cn";
import { IdpAccountItems } from "./idp-account-items";
import { UserAvatar } from "./user-avatar";

const ICON = { size: 16, "aria-hidden": true } as const;

const TRIGGER = cn(buttonClassName({ variant: "ghost" }), "px-2");

export interface AccountMenuProps {
	user: CurrentUser;
	logoutBusy: boolean;
	onLogout: () => void;
	/** Avatar and chevron only: the email stays in the menu and the label. */
	compact?: boolean;
}

/** Avatar button and the account menu of the signed-in top bar. */
export function AccountMenu({
	user,
	logoutBusy,
	onLogout,
	compact = false,
}: AccountMenuProps) {
	const t = useT();
	return (
		<DropdownMenu>
			<DropdownMenuTrigger asChild>
				<button
					type="button"
					data-id="topbar-account"
					aria-label={t("layout.accountMenu.trigger", { email: user.email })}
					className={TRIGGER}
				>
					<UserAvatar user={user} />
					{compact ? null : (
						<span className="max-w-56 truncate font-mono text-[0.8125rem] text-muted max-lg:hidden">
							{user.email}
						</span>
					)}
					<IconChevronDown size={16} aria-hidden="true" />
				</button>
			</DropdownMenuTrigger>
			<DropdownMenuContent data-id="account-menu">
				<DropdownMenuLabel className="truncate font-semibold text-fg">
					{user.email}
				</DropdownMenuLabel>
				<DropdownMenuSeparator />
				<IdpAccountItems />
				<DropdownMenuSeparator />
				<DropdownMenuItem
					data-id="topbar-logout"
					icon={<IconLogout {...ICON} />}
					disabled={logoutBusy}
					onSelect={onLogout}
				>
					{t("layout.accountMenu.signOut")}
				</DropdownMenuItem>
			</DropdownMenuContent>
		</DropdownMenu>
	);
}
