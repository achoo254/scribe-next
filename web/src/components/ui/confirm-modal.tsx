import { IconAlertTriangle } from "@tabler/icons-react";
import { type ReactNode, useRef, useState } from "react";
import { errorMessage } from "../../lib/api";
import { Alert } from "./alert";
import { Button } from "./button";
import { Modal, ModalClose } from "./modal";
import { ModalFooter } from "./modal-footer";

export interface ConfirmModalProps {
	open: boolean;
	onOpenChange: (open: boolean) => void;
	title: ReactNode;
	/** Bold first line of the warning, e.g. `Xoá “Bản ghi 1”?`. */
	question: ReactNode;
	/** Consequences under the question (summary, "can't be undone"…). */
	children?: ReactNode;
	confirmLabel: string;
	/** Runs the action; the modal closes when it resolves, shows the error if it throws. */
	onConfirm: () => Promise<void>;
	/** Keeps the confirm button off, e.g. while a dry run is still loading. */
	confirmDisabled?: boolean;
	/** See `Modal`: where focus goes when the trigger no longer exists. */
	onCloseAutoFocus?: (event: Event) => void;
	"data-id"?: string;
}

/**
 * Destructive confirmation: danger warning, Cancel / confirm footer. Keeps the
 * modal open with the error message when the action fails.
 */
export function ConfirmModal({
	open,
	onOpenChange,
	title,
	question,
	children,
	confirmLabel,
	onConfirm,
	confirmDisabled = false,
	onCloseAutoFocus,
	"data-id": dataId,
}: ConfirmModalProps) {
	const [busy, setBusy] = useState(false);
	// Destructive dialogs open on the safe choice.
	const cancel = useRef<HTMLButtonElement>(null);
	const [error, setError] = useState<unknown>(null);

	function changeOpen(next: boolean) {
		if (busy) return;
		if (!next) setError(null);
		onOpenChange(next);
	}

	async function confirm() {
		setBusy(true);
		setError(null);
		try {
			await onConfirm();
			onOpenChange(false);
		} catch (cause) {
			setError(cause);
		} finally {
			setBusy(false);
		}
	}

	return (
		<Modal
			open={open}
			onOpenChange={changeOpen}
			title={title}
			size="sm"
			initialFocusRef={cancel}
			onCloseAutoFocus={onCloseAutoFocus}
			data-id={dataId}
			footer={
				<ModalFooter>
					<ModalClose asChild>
						<Button ref={cancel} variant="ghost" disabled={busy}>
							Huỷ
						</Button>
					</ModalClose>
					<Button
						variant="danger"
						disabled={confirmDisabled}
						loading={busy}
						onClick={() => void confirm()}
						data-id={dataId ? `${dataId}-confirm` : undefined}
					>
						{confirmLabel}
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
				<div className="grid min-w-0 gap-1">
					<strong className="block font-mono">{question}</strong>
					{children}
				</div>
			</div>
			{error ? <Alert tone="danger">{errorMessage(error)}</Alert> : null}
		</Modal>
	);
}
