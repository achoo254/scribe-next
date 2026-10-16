/* Copied from localman-next apps/web/src/components/layout/account-controls.tsx.
   Scribe change: signs out through auth-context (`useSignOut`) instead of
   Localman's logout form, and asks first while a recording or an upload
   would be cut short; links and unlinks chat.hoandat.com. */
import { useState } from "react";
import type { CurrentUser } from "../auth/auth-api";
import { ChatLinkDialog } from "../components/chat-link-dialog";
import { ConfirmModal } from "../components/ui/confirm-modal";
import { useToast } from "../components/ui/toast";
import {
	useChatLinkQuery,
	useUnlinkChat,
} from "../features/chat-link/chat-link-queries";
import { useT } from "../i18n/i18n-context";
import { errorMessage } from "../lib/api";
import { AccountMenu } from "./account-menu";
import {
	IDLE_ACTIVITY,
	type SessionActivity,
	isSessionBusy,
} from "./session-activity";
import { useSignOut } from "./use-sign-out";

export interface AccountControlsProps {
	user: CurrentUser;
	/** What signing out would interrupt (see session-activity.ts). */
	activity?: SessionActivity;
	/** Saves running browser recordings before the session is deleted. */
	beforeSignOut?: () => Promise<void>;
	/** Avatar-only trigger (see `AccountMenu`). */
	compact?: boolean;
}

/** Account menu of the signed-in user with its sign-out confirmation. */
export function AccountControls({
	user,
	activity = IDLE_ACTIVITY,
	beforeSignOut,
	compact = false,
}: AccountControlsProps) {
	const t = useT();
	const toast = useToast();
	const session = useSignOut();
	const [confirming, setConfirming] = useState(false);
	// Minutes are made only by accounts that may transcribe; others have nothing to send.
	const canSend = user.canTranscribe === true;
	const chatLink = useChatLinkQuery(canSend);
	const unlink = useUnlinkChat();
	const [linking, setLinking] = useState(false);

	function unlinkChat(): void {
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

	return (
		<>
			<AccountMenu
				user={user}
				chat={
					canSend
						? {
								linked: chatLink.data?.linked === true,
								linkedAt: chatLink.data?.linkedAt ?? null,
								// Unknown (loading, failed): offer linking; the dialog says why not.
								enabled: chatLink.data?.enabled ?? true,
								busy: unlink.isPending,
								onLink: () => setLinking(true),
								onUnlink: unlinkChat,
							}
						: undefined
				}
				logoutBusy={session.busy}
				onLogout={requestSignOut}
				compact={compact}
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
