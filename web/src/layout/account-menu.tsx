/* Copied from localman-next apps/web/src/components/layout/account-menu.tsx.
   Scribe change: no in-app account, notification, docs, home or admin
   entries (Scribe has none of those pages) and no feature flags; the
   trigger names the email, the only identity the session carries; the
   chat.hoandat.com link entry (where the minutes are sent). */
import {
	IconChevronDown,
	IconLogout,
	IconMessageCircle,
	IconMessageOff,
} from "@tabler/icons-react";
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
import { formatTimeAndDay } from "../lib/format";
import { IdpAccountItems } from "./idp-account-items";
import { UserAvatar } from "./user-avatar";

const ICON = { size: 16, "aria-hidden": true } as const;

const TRIGGER = cn(buttonClassName({ variant: "ghost" }), "px-2");

/**
 * The chat.hoandat.com entry: link (when the server takes links), or
 * unlink once linked.
 */
export interface AccountMenuChat {
	linked: boolean;
	/** When the link was confirmed (ISO), shown above "Gỡ liên kết chat". */
	linkedAt: string | null;
	/** Who the chat is (name on chat.hoandat.com), when the server knows. */
	chatName: string | null;
	/** The server takes new links. */
	enabled: boolean;
	busy: boolean;
	onLink: () => void;
	onUnlink: () => void;
}

export interface AccountMenuProps {
	user: CurrentUser;
	/** Absent for accounts that cannot make minutes (nothing to send). */
	chat?: AccountMenuChat;
	logoutBusy: boolean;
	onLogout: () => void;
	/** Avatar and chevron only: the email stays in the menu and the label. */
	compact?: boolean;
}

/** Avatar button and the account menu of the signed-in top bar. */
export function AccountMenu({
	user,
	chat,
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
				{chat && (chat.linked || chat.enabled) ? (
					<>
						{chat.linked && chat.chatName ? (
							<DropdownMenuLabel
								data-id="account-menu-chat-name"
								className="truncate text-muted text-xs"
							>
								{t("chat.menu.linkedWith", { name: chat.chatName })}
							</DropdownMenuLabel>
						) : null}
						{chat.linked && chat.linkedAt ? (
							<DropdownMenuLabel
								data-id="account-menu-chat-since"
								className="text-muted text-xs"
							>
								{t("chat.menu.linkedSince", {
									time: formatTimeAndDay(chat.linkedAt),
								})}
							</DropdownMenuLabel>
						) : null}
						{chat.linked ? (
							<DropdownMenuItem
								data-id="account-menu-chat-unlink"
								icon={<IconMessageOff {...ICON} />}
								disabled={chat.busy}
								onSelect={chat.onUnlink}
							>
								{t("chat.menu.unlink")}
							</DropdownMenuItem>
						) : (
							<DropdownMenuItem
								data-id="account-menu-chat-link"
								icon={<IconMessageCircle {...ICON} />}
								onSelect={chat.onLink}
							>
								{t("chat.menu.link")}
							</DropdownMenuItem>
						)}
						<DropdownMenuSeparator />
					</>
				) : null}
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
