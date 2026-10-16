import { IconExternalLink } from "@tabler/icons-react";
import { useEffect, useState } from "react";
import {
	useChatLinkQuery,
	useStartChatLink,
} from "../features/chat-link/chat-link-queries";
import { useT } from "../i18n/i18n-context";
import { errorMessage } from "../lib/api";
import { openExternal } from "../lib/external-link";
import { formatTimeAndDay } from "../lib/format";
import { Alert } from "./ui/alert";
import { Button } from "./ui/button";
import { Modal, ModalClose } from "./ui/modal";
import { ModalFooter } from "./ui/modal-footer";

export interface ChatLinkDialogProps {
	open: boolean;
	onOpenChange: (open: boolean) => void;
}

/**
 * How to link chat.hoandat.com: a one-time deep link (made when the dialog
 * opens) that the user opens and confirms with "Gửi /start". The link is
 * opened from the button's own click, so the browser does not block the
 * new tab, and through `openExternal`, so the iPhone app hands it to the
 * system browser. The user then sends `/start` and presses "Xác nhận liên
 * kết" in the chat; the status is read again when the page gets the focus
 * back and every few seconds meanwhile (`useChatLinkQuery`); once linked,
 * the dialog says so.
 */
export function ChatLinkDialog({ open, onOpenChange }: ChatLinkDialogProps) {
	const t = useT();
	const start = useStartChatLink();
	const [opened, setOpened] = useState(false);
	// Linking ends when the user presses "Xác nhận liên kết" in the chat:
	// poll until then, as long as the token lives.
	const status = useChatLinkQuery(
		open,
		opened ? (start.data?.expiresAt ?? null) : null,
	);
	const linked = status.data?.linked === true;
	const { mutate, reset, isIdle } = start;

	// A fresh token per opening (an old one may have expired or been used),
	// and none when the account turns out to be linked already.
	const known = !status.isPending;
	useEffect(() => {
		if (open && known && !linked && isIdle) mutate();
	}, [open, known, linked, isIdle, mutate]);
	useEffect(() => {
		if (open) return;
		reset();
		setOpened(false);
	}, [open, reset]);

	const url = start.data?.url ?? null;

	return (
		<Modal
			open={open}
			onOpenChange={onOpenChange}
			title={linked ? t("chat.dialog.linkedTitle") : t("chat.dialog.title")}
			size="sm"
			data-id="chat-link-dialog"
			footer={
				linked ? (
					<ModalFooter>
						<ModalClose asChild>
							<Button variant="primary" data-id="chat-link-done">
								{t("common.actions.close")}
							</Button>
						</ModalClose>
					</ModalFooter>
				) : (
					<ModalFooter>
						<ModalClose asChild>
							<Button variant="ghost">{t("common.actions.cancel")}</Button>
						</ModalClose>
						<Button
							variant="primary"
							data-id="chat-link-open"
							loading={start.isPending}
							disabled={url === null}
							leadingIcon={<IconExternalLink size={16} aria-hidden="true" />}
							onClick={() => {
								if (url && openExternal(url)) setOpened(true);
							}}
						>
							{t("chat.dialog.open")}
							<span className="sr-only">{t("chat.dialog.opensBrowser")}</span>
						</Button>
					</ModalFooter>
				)
			}
		>
			{linked ? (
				<Alert tone="success" data-id="chat-link-linked">
					{t("chat.dialog.linked")}
				</Alert>
			) : (
				<div className="grid gap-4 text-sm">
					<p className="m-0">{t("chat.dialog.intro")}</p>
					<ol
						aria-label={t("chat.dialog.steps")}
						className="m-0 grid list-decimal gap-1.5 pl-5"
					>
						<li>{t("chat.dialog.step1")}</li>
						<li>{t("chat.dialog.step2")}</li>
						<li>{t("chat.dialog.step3")}</li>
						<li>{t("chat.dialog.step4")}</li>
					</ol>
					{start.isError ? (
						<div className="grid justify-items-start gap-2">
							<Alert tone="danger" data-id="chat-link-error">
								{errorMessage(start.error)}
							</Alert>
							<Button
								size="sm"
								data-id="chat-link-retry"
								onClick={() => mutate()}
							>
								{t("common.actions.retry")}
							</Button>
						</div>
					) : start.data ? (
						<p className="m-0 text-muted" data-id="chat-link-expires">
							{t("chat.dialog.expires", {
								time: formatTimeAndDay(start.data.expiresAt),
							})}
						</p>
					) : (
						<p className="m-0 text-muted" aria-live="polite">
							{t("chat.dialog.preparing")}
						</p>
					)}
					{opened ? (
						<p className="m-0 text-muted" aria-live="polite">
							{t("chat.dialog.waiting")}
						</p>
					) : null}
				</div>
			)}
		</Modal>
	);
}
