/* The account menu is the shared one of every hoandat.com app
   (`AccountMenu` of @achoo254/app-shell: IdP picture or initial, name,
   the IdP account pages, sign-out). Scribe adds its own entries: "Key
   riêng", linking chat.hoandat.com (where the minutes are sent), the
   privacy policy and terms, "Xoá tài khoản" (App Store guideline
   5.1.1(v)). Signing out goes through auth-context (`useSignOut`) and asks
   first while a recording or an upload would be cut short. */
import { AccountMenu, type AccountMenuEntry } from "@achoo254/app-shell";
import {
	IconFileText,
	IconKey,
	IconMessageCircle,
	IconMessageOff,
	IconShieldLock,
	IconTrash,
} from "@tabler/icons-react";
import { useState } from "react";
import type { CurrentUser } from "../auth/auth-api";
import { ChatLinkDialog } from "../components/chat-link-dialog";
import { DeleteAccountDialog } from "../components/delete-account-dialog";
import { ConfirmModal } from "../components/ui/confirm-modal";
import { useToast } from "../components/ui/toast";
import { UserKeysDialog } from "../components/user-keys-dialog";
import {
	useChatLinkQuery,
	useUnlinkChat,
} from "../features/chat-link/chat-link-queries";
import { useI18n } from "../i18n/i18n-context";
import { errorMessage, isNativeShell } from "../lib/api";
import { openExternal } from "../lib/external-link";
import { IDP_ACCOUNT_URL } from "../lib/idp";
import { PATHS } from "./app-paths";
import { RouterLink, SHELL_BRAND } from "./scribe-public-page";
import {
	IDLE_ACTIVITY,
	type SessionActivity,
	isSessionBusy,
} from "./session-activity";
import { useSignOut } from "./use-sign-out";

/** 16px decorative icon of an entry, tinted like the shell's own. */
const ICON = {
	size: 16,
	"aria-hidden": true,
	className: "shell-icon",
} as const;

export interface AccountControlsProps {
	user: CurrentUser;
	/** What signing out would interrupt (see session-activity.ts). */
	activity?: SessionActivity;
	/** Saves running browser recordings before the session is deleted. */
	beforeSignOut?: () => Promise<void>;
}

/** Account menu of the signed-in user, its dialogs and sign-out confirmation. */
export function AccountControls({
	user,
	activity = IDLE_ACTIVITY,
	beforeSignOut,
}: AccountControlsProps) {
	const { locale, t } = useI18n();
	const toast = useToast();
	const session = useSignOut();
	const [confirming, setConfirming] = useState(false);
	// Minutes are made only by accounts that may transcribe; others have nothing to send.
	const canSend = user.canTranscribe === true;
	const chatLink = useChatLinkQuery(canSend);
	const unlink = useUnlinkChat();
	const [linking, setLinking] = useState(false);
	const [keysOpen, setKeysOpen] = useState(false);
	const [deleting, setDeleting] = useState(false);

	function unlinkChat(): void {
		if (unlink.isPending) return;
		unlink.mutate(undefined, {
			onSuccess: () => toast.show({ title: t("chat.menu.unlinked") }),
			onError: (cause) =>
				toast.show({
					tone: "error",
					title: t("chat.menu.unlinkFailed"),
					description: errorMessage(cause),
				}),
		});
	}

	function requestSignOut(): void {
		if (isSessionBusy(activity)) {
			setConfirming(true);
			return;
		}
		void session.signOut(beforeSignOut);
	}

	const linked = chatLink.data?.linked === true;
	const chatName = chatLink.data?.chatName ?? null;
	// Unknown (loading, failed): offer linking; the dialog says why not.
	const linkEnabled = chatLink.data?.enabled ?? true;
	const chatEntries: AccountMenuEntry[] = !canSend
		? []
		: linked
			? [
					{
						label: chatName
							? t("chat.menu.unlinkFrom", { name: chatName })
							: t("chat.menu.unlink"),
						dataId: "account-menu-chat-unlink",
						icon: <IconMessageOff {...ICON} />,
						onClick: unlinkChat,
					},
				]
			: linkEnabled
				? [
						{
							label: t("chat.menu.link"),
							dataId: "account-menu-chat-link",
							icon: <IconMessageCircle {...ICON} />,
							onClick: () => setLinking(true),
						},
					]
				: [];
	const entries: AccountMenuEntry[][] = [
		[
			{
				label: t("keys.menu"),
				dataId: "account-menu-keys",
				icon: <IconKey {...ICON} />,
				onClick: () => setKeysOpen(true),
			},
		],
		chatEntries,
		[
			{
				label: t("legal.nav.privacy"),
				dataId: "account-menu-privacy",
				icon: <IconShieldLock {...ICON} />,
				href: PATHS.privacy,
			},
			{
				label: t("legal.nav.terms"),
				dataId: "account-menu-terms",
				icon: <IconFileText {...ICON} />,
				href: PATHS.terms,
			},
		],
		[
			{
				label: t("account.menu"),
				dataId: "account-menu-delete",
				icon: <IconTrash {...ICON} />,
				onClick: () => setDeleting(true),
			},
		],
	];

	return (
		<>
			<AccountMenu
				user={user}
				locale={locale}
				domain={SHELL_BRAND.domain}
				idpAccountUrl={IDP_ACCOUNT_URL}
				entries={entries}
				signOut={{ onClick: requestSignOut, busy: session.busy }}
				// The apps open the IdP pages in the system browser (Capacitor
				// hands `window.open` over); the web keeps the new-tab link.
				onOpenExternal={isNativeShell() ? openExternal : undefined}
				linkComponent={RouterLink}
			/>
			<UserKeysDialog open={keysOpen} onOpenChange={setKeysOpen} />
			<DeleteAccountDialog
				open={deleting}
				onOpenChange={setDeleting}
				email={user.email}
				busy={isSessionBusy(activity)}
			/>
			{canSend ? (
				<ChatLinkDialog open={linking} onOpenChange={setLinking} />
			) : null}
			<ConfirmModal
				open={confirming}
				onOpenChange={setConfirming}
				title={t("layout.signOutConfirm.title")}
				question={t("layout.signOutConfirm.question")}
				confirmLabel={t("layout.signOutConfirm.confirm")}
				onConfirm={() => session.signOut(beforeSignOut)}
				data-id="sign-out-confirm"
			>
				<ul className="m-0 grid list-disc gap-1 pl-5 text-sm">
					{activity.recording ? (
						<li>{t("layout.signOutConfirm.recording")}</li>
					) : null}
					{activity.segmentsWaiting > 0 ? (
						<li>
							{t("layout.signOutConfirm.segmentsWaiting", {
								count: activity.segmentsWaiting,
							})}
						</li>
					) : null}
					{activity.fileUploading ? (
						<li>{t("layout.signOutConfirm.fileUploading")}</li>
					) : null}
				</ul>
			</ConfirmModal>
		</>
	);
}
