import { type ReactNode, useState } from "react";
import { useAuth } from "../../auth/auth-context";
import { Button } from "../../components/ui/button";
import { Modal, ModalClose } from "../../components/ui/modal";
import { ModalFooter } from "../../components/ui/modal-footer";
import { useToast } from "../../components/ui/toast";
import { useRequestTranscripts } from "../../features/transcript/transcript-queries";
import { useT } from "../../i18n/i18n-context";
import { type TagRenderers, renderRich } from "../../i18n/rich-text";
import { errorMessage } from "../../lib/api";

/** `<strong>` in the confirmation texts: the minutes stand out. */
const STRONG: TagRenderers = { strong: (chunk) => <strong>{chunk}</strong> };

/** Above this many minutes, say how much of the month it uses before sending. */
const CONFIRM_ABOVE_MINUTES = 10;

export interface Transcribe {
	/**
	 * Queues the files; `seconds` is their total length, used to ask first
	 * when a long recording takes a big part of the monthly minutes.
	 */
	start: (fileIds: readonly string[], seconds: number) => Promise<void>;
	pending: boolean;
	/** The confirmation dialog; render it once next to the buttons that call `start`. */
	dialog: ReactNode;
}

/**
 * "Chép lời" on the server's OpenAI key, shared by the recordings list and
 * the transcript reader. `onSent` runs once the backend queued the files;
 * the minutes left are re-read afterwards.
 */
export function useTranscribe(onSent?: () => void): Transcribe {
	const t = useT();
	const toast = useToast();
	const { user, refresh } = useAuth();
	const request = useRequestTranscripts();
	const [pendingConfirm, setPendingConfirm] = useState<{
		fileIds: readonly string[];
		minutes: number;
	} | null>(null);
	// On a tenant's domain runs use the owner's key: no Scribe minutes to count.
	const quota =
		user?.transcription?.keySource === "tenant" ? null : user?.transcription;
	const left = quota
		? Math.max(0, quota.limitMinutes - quota.usedMinutes)
		: null;

	async function send(fileIds: readonly string[]): Promise<void> {
		try {
			await request.mutateAsync({ fileIds });
			onSent?.();
		} catch (cause) {
			toast.show({
				tone: "error",
				title: t("transcript.transcribe.failed"),
				description: errorMessage(cause),
			});
		} finally {
			void refresh();
		}
	}

	async function start(
		fileIds: readonly string[],
		seconds: number,
	): Promise<void> {
		const minutes = Math.ceil(seconds / 60);
		if (minutes > CONFIRM_ABOVE_MINUTES) {
			setPendingConfirm({ fileIds, minutes });
			return;
		}
		await send(fileIds);
	}

	const confirmMinutes = pendingConfirm?.minutes ?? 0;
	const dialog = (
		<Modal
			open={pendingConfirm !== null}
			onOpenChange={(next) => {
				if (!next) setPendingConfirm(null);
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
						disabled={left !== null && confirmMinutes > left}
						loading={request.isPending}
						onClick={() => {
							const next = pendingConfirm;
							setPendingConfirm(null);
							if (next) void send(next.fileIds);
						}}
					>
						{t("transcript.transcribe.send")}
					</Button>
				</ModalFooter>
			}
		>
			<p className="m-0 text-sm">
				{renderRich(
					left !== null
						? t("transcript.transcribe.usageOfLeft", {
								minutes: confirmMinutes,
								left,
							})
						: t("transcript.transcribe.usage", { minutes: confirmMinutes }),
					STRONG,
				)}
			</p>
			{left !== null && confirmMinutes > left ? (
				<p className="m-0 mt-2 text-error-text text-sm">
					{t("transcript.transcribe.notEnough")}
				</p>
			) : null}
		</Modal>
	);

	return { start, pending: request.isPending, dialog };
}
