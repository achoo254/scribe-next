import { type ReactNode, useState } from "react";
import { useAuth } from "../../auth/auth-context";
import { providerNameKey } from "../../components/key-providers";
import { Button } from "../../components/ui/button";
import { Modal, ModalClose } from "../../components/ui/modal";
import { ModalFooter } from "../../components/ui/modal-footer";
import { useToast } from "../../components/ui/toast";
import { NeedsKeyNotice } from "../../components/user-keys-entry";
import { useRequestTranscripts } from "../../features/transcript/transcript-queries";
import { isOwnKeyRefusal, ownKeyFor } from "../../features/user-keys/own-keys";
import { useT } from "../../i18n/i18n-context";
import { type TagRenderers, renderRich } from "../../i18n/rich-text";
import { errorMessage } from "../../lib/api";

/** `<strong>` in the confirmation text: the minutes stand out. */
const STRONG: TagRenderers = { strong: (chunk) => <strong>{chunk}</strong> };

/** Above this many minutes, say how long it is before sending. */
const CONFIRM_ABOVE_MINUTES = 10;

export interface Transcribe {
	/**
	 * Queues the files; `seconds` is their total length, used to ask first
	 * when a long recording goes to the user's provider.
	 */
	start: (fileIds: readonly string[], seconds: number) => Promise<void>;
	pending: boolean;
	/** The dialogs; render them once next to the buttons that call `start`. */
	dialog: ReactNode;
}

/** What the dialog asks or explains; null when closed. */
type Prompt =
	| { kind: "confirm"; fileIds: readonly string[]; minutes: number }
	| { kind: "needsKey"; message: string };

/**
 * "Chép lời" on the user's own transcription key, shared by the recordings
 * list and the transcript reader. Without a key (or when the server says
 * so), a dialog explains it and opens "Key riêng". `onSent` runs once the
 * backend queued the files; `/api/me` is re-read afterwards (minutes on
 * the user's key).
 */
export function useTranscribe(onSent?: () => void): Transcribe {
	const t = useT();
	const toast = useToast();
	const { user, refresh } = useAuth();
	const request = useRequestTranscripts();
	const [prompt, setPrompt] = useState<Prompt | null>(null);
	const provider = ownKeyFor(user, "transcribe");
	const nameKey = providerNameKey(provider);
	const providerName = nameKey ? t(nameKey) : (provider ?? "");

	async function send(fileIds: readonly string[]): Promise<void> {
		try {
			await request.mutateAsync({ fileIds });
			onSent?.();
		} catch (cause) {
			if (isOwnKeyRefusal(cause)) {
				setPrompt({ kind: "needsKey", message: errorMessage(cause) });
			} else {
				toast.show({
					tone: "error",
					title: t("transcript.transcribe.failed"),
					description: errorMessage(cause),
				});
			}
		} finally {
			void refresh();
		}
	}

	async function start(
		fileIds: readonly string[],
		seconds: number,
	): Promise<void> {
		if (provider === null) {
			setPrompt({ kind: "needsKey", message: t("keys.needed.transcribe") });
			return;
		}
		const minutes = Math.ceil(seconds / 60);
		if (minutes > CONFIRM_ABOVE_MINUTES) {
			setPrompt({ kind: "confirm", fileIds, minutes });
			return;
		}
		await send(fileIds);
	}

	const confirm = prompt?.kind === "confirm" ? prompt : null;
	const needsKey = prompt?.kind === "needsKey" ? prompt : null;
	const close = () => setPrompt(null);
	const dialog = (
		<>
			<Modal
				open={confirm !== null}
				onOpenChange={(next) => {
					if (!next) close();
				}}
				title={t("transcript.transcribe.confirmTitle")}
				size="sm"
				data-id="transcribe-confirm"
				footer={
					<ModalFooter>
						<ModalClose asChild>
							<Button variant="ghost">{t("common.actions.cancel")}</Button>
						</ModalClose>
						<Button
							variant="primary"
							data-id="transcribe-confirm-send"
							loading={request.isPending}
							onClick={() => {
								close();
								if (confirm) void send(confirm.fileIds);
							}}
						>
							{t("transcript.transcribe.send")}
						</Button>
					</ModalFooter>
				}
			>
				<p className="m-0 text-sm">
					{renderRich(
						t("transcript.transcribe.usage", {
							minutes: confirm?.minutes ?? 0,
							provider: providerName,
						}),
						STRONG,
					)}
				</p>
			</Modal>
			<Modal
				open={needsKey !== null}
				onOpenChange={(next) => {
					if (!next) close();
				}}
				title={t("keys.needed.title")}
				size="sm"
				data-id="transcribe-needs-key"
				footer={
					<ModalFooter>
						<ModalClose asChild>
							<Button variant="ghost">{t("common.actions.close")}</Button>
						</ModalClose>
					</ModalFooter>
				}
			>
				<NeedsKeyNotice
					data-id="transcribe-needs-key-notice"
					message={needsKey?.message ?? ""}
					onOpen={close}
				/>
			</Modal>
		</>
	);

	return { start, pending: request.isPending, dialog };
}
