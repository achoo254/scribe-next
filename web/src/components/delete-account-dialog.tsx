import { IconAlertTriangle } from "@tabler/icons-react";
import { useEffect, useState } from "react";
import { useAuth } from "../auth/auth-context";
import { confirmMatches, deleteAccount } from "../features/account/account-api";
import { useT } from "../i18n/i18n-context";
import { renderRich } from "../i18n/rich-text";
import { TEXT_LINK } from "../layout/legal-links";
import { errorCode, errorMessage } from "../lib/api";
import { IDP_ACCOUNT_URL } from "../lib/idp";
import { Alert } from "./ui/alert";
import { Button } from "./ui/button";
import { FormField } from "./ui/form-field";
import { Input } from "./ui/input";
import { Modal, ModalClose } from "./ui/modal";
import { ModalFooter } from "./ui/modal-footer";

export interface DeleteAccountDialogProps {
	open: boolean;
	onOpenChange: (open: boolean) => void;
	email: string;
	/** A recording or an upload is running on this device: deleting waits. */
	busy: boolean;
}

/**
 * Deletes the Scribe account in the app (App Store guideline 5.1.1(v)):
 * says exactly what goes, that the hoandat account stays (with the link
 * to delete it at hoandat-auth), and asks the user to type "XOÁ". Once
 * deleted, the device signs out locally and the sign-in page says so.
 */
export function DeleteAccountDialog({
	open,
	onOpenChange,
	email,
	busy,
}: DeleteAccountDialogProps) {
	const t = useT();
	const { forgetDeletedAccount } = useAuth();
	const [typed, setTyped] = useState("");
	const [working, setWorking] = useState(false);
	const [error, setError] = useState<string | null>(null);

	useEffect(() => {
		if (open) return;
		setTyped("");
		setError(null);
	}, [open]);

	function changeOpen(next: boolean): void {
		if (!working) onOpenChange(next);
	}

	async function remove(): Promise<void> {
		setWorking(true);
		setError(null);
		try {
			const result = await deleteAccount(typed);
			// Signed out: the app goes to /login, which says the account is gone.
			await forgetDeletedAccount(result.complete ? "done" : "pending");
		} catch (cause) {
			setError(
				errorCode(cause) === "RECORDING_OPEN"
					? t("account.delete.recordingOpen")
					: errorMessage(cause),
			);
			setWorking(false);
		}
	}

	const word = t("account.delete.confirmWord");
	const ready = confirmMatches(typed) && !busy;

	return (
		<Modal
			open={open}
			onOpenChange={changeOpen}
			title={t("account.delete.title")}
			size="md"
			data-id="delete-account-dialog"
			footer={
				<ModalFooter>
					<ModalClose asChild>
						<Button variant="ghost" disabled={working}>
							{t("common.actions.cancel")}
						</Button>
					</ModalClose>
					<Button
						variant="danger"
						disabled={!ready}
						loading={working}
						onClick={() => void remove()}
						data-id="delete-account-confirm"
					>
						{t("account.delete.confirm")}
					</Button>
				</ModalFooter>
			}
		>
			<div
				data-tone="danger"
				className="tint-soft flex gap-3 rounded-md border px-4 py-3 text-fg text-sm [--tone:var(--c-error)]"
			>
				<IconAlertTriangle
					size={20}
					aria-hidden="true"
					className="shrink-0 text-error"
				/>
				<div className="grid min-w-0 gap-2">
					<strong className="block break-words font-mono">
						{t("account.delete.question", { email })}
					</strong>
					<p className="m-0">{t("account.delete.intro")}</p>
					<ul
						data-id="delete-account-scope"
						className="m-0 grid list-disc gap-1 pl-5"
					>
						<li>{t("account.delete.files")}</li>
						<li>{t("account.delete.transcripts")}</li>
						<li>{t("account.delete.keys")}</li>
						<li>{t("account.delete.sessions")}</li>
					</ul>
					<p className="m-0 font-semibold">{t("account.delete.undo")}</p>
				</div>
			</div>
			<p className="m-0 text-muted text-sm">{t("account.delete.audit")}</p>
			<p className="m-0 text-sm">
				{renderRich(t("account.delete.idp"), {
					// In the iOS app Capacitor opens target=_blank links in Safari.
					account: (chunk) => (
						<a
							href={IDP_ACCOUNT_URL}
							target="_blank"
							rel="noopener noreferrer"
							className={TEXT_LINK}
						>
							{chunk}
						</a>
					),
				})}
			</p>
			{busy ? (
				<Alert tone="warning" data-id="delete-account-busy">
					{t("account.delete.busy")}
				</Alert>
			) : null}
			<FormField label={t("account.delete.confirmLabel", { word })}>
				<Input
					value={typed}
					onChange={(event) => setTyped(event.target.value)}
					autoComplete="off"
					autoCapitalize="characters"
					autoCorrect="off"
					spellCheck={false}
					disabled={busy || working}
					data-id="delete-account-input"
				/>
			</FormField>
			{error ? (
				<Alert tone="danger" data-id="delete-account-error">
					{error}
				</Alert>
			) : null}
		</Modal>
	);
}
