import * as Dialog from "@radix-ui/react-dialog";
import { IconX } from "@tabler/icons-react";
import { type ReactNode, type RefObject, useRef } from "react";
import { useT } from "../../i18n/i18n-context";
import { cn } from "../../lib/cn";
import { IconButton } from "./icon-button";

export type ModalSize = "sm" | "md" | "lg" | "xl";

/** Max widths from the prototype: 384 / 448 / 576 / 768px. */
const SIZE_CLASSES: Record<ModalSize, string> = {
	sm: "max-w-sm",
	md: "max-w-md",
	lg: "max-w-xl",
	xl: "max-w-3xl",
};

export interface ModalProps {
	open?: boolean;
	defaultOpen?: boolean;
	onOpenChange?: (open: boolean) => void;
	/** Element that opens the modal (rendered via Radix `asChild`). */
	trigger?: ReactNode;
	title: ReactNode;
	description?: ReactNode;
	size?: ModalSize;
	/** Usually a <ModalFooter>; stays pinned below the scrolling body. */
	footer?: ReactNode;
	children?: ReactNode;
	/** Name of the close button; "Close" in the current language by default. */
	closeLabel?: string;
	/**
	 * What receives focus on open: an element ref, or "dialog" for the frame
	 * itself (read-only content). Falls back to the frame while the ref is
	 * still empty (e.g. a lazily loaded body). Default: Radix picks the first
	 * focusable element.
	 */
	initialFocusRef?: RefObject<HTMLElement | null> | "dialog";
	/**
	 * Runs when the modal closes, before focus returns to the trigger; call
	 * `event.preventDefault()` and focus something else when the trigger is
	 * gone (e.g. the row it belonged to was removed).
	 */
	onCloseAutoFocus?: (event: Event) => void;
	"data-id"?: string;
}

/**
 * Escape inside an expanded combobox closes its list, not the modal (the
 * combobox handles the key itself after Radix's document listener).
 */
function keepOpenForExpandedCombobox(event: KeyboardEvent): void {
	const target = event.target;
	if (
		target instanceof HTMLElement &&
		target.getAttribute("role") === "combobox" &&
		target.getAttribute("aria-expanded") === "true"
	) {
		event.preventDefault();
	}
}

/** Re-exported so footers can close the modal: `<ModalClose asChild>…`. */
export const ModalClose = Dialog.Close;

export function Modal({
	open,
	defaultOpen,
	onOpenChange,
	trigger,
	title,
	description,
	size = "md",
	footer,
	children,
	closeLabel,
	initialFocusRef,
	onCloseAutoFocus,
	"data-id": dataId,
}: ModalProps) {
	const t = useT();
	const content = useRef<HTMLDivElement>(null);

	function focusInitial(event: Event) {
		if (!initialFocusRef) return;
		event.preventDefault();
		const target =
			initialFocusRef === "dialog" ? null : initialFocusRef.current;
		(target ?? content.current)?.focus();
	}

	return (
		<Dialog.Root
			open={open}
			defaultOpen={defaultOpen}
			onOpenChange={onOpenChange}
		>
			{trigger ? <Dialog.Trigger asChild>{trigger}</Dialog.Trigger> : null}
			<Dialog.Portal>
				<Dialog.Overlay
					className={cn(
						"fixed inset-0 z-100 flex items-start justify-center overflow-y-auto bg-backdrop",
						"p-gutter pt-(--modal-offset-top)",
						"data-[state=closed]:animate-fade-out data-[state=open]:animate-fade-in",
					)}
				>
					<Dialog.Content
						ref={content}
						onOpenAutoFocus={focusInitial}
						onCloseAutoFocus={onCloseAutoFocus}
						onEscapeKeyDown={keepOpenForExpandedCombobox}
						data-id={dataId}
						data-size={size}
						// Without a description, opt out of Radix's describedby wiring.
						{...(description ? {} : { "aria-describedby": undefined })}
						className={cn(
							"relative flex w-full flex-col overflow-hidden rounded-lg border border-border bg-surface text-fg shadow-lg",
							"max-h-[calc(100dvh-var(--modal-offset-top)-var(--gutter))] focus:outline-none",
							"data-[state=closed]:animate-modal-out data-[state=open]:animate-modal-in",
							SIZE_CLASSES[size],
						)}
					>
						<div className="flex items-center justify-between gap-3 border-border-subtle border-b py-3 pr-3 pl-6">
							<Dialog.Title className="truncate font-bold font-mono text-base">
								{title}
							</Dialog.Title>
							<Dialog.Close asChild>
								<IconButton
									label={closeLabel ?? t("common.actions.close")}
									icon={<IconX size={18} aria-hidden="true" />}
								/>
							</Dialog.Close>
						</div>
						<div className="grid flex-1 grid-cols-[minmax(0,1fr)] content-start gap-6 overflow-y-auto p-6">
							{description ? (
								<Dialog.Description className="text-muted text-sm">
									{description}
								</Dialog.Description>
							) : null}
							{children}
						</div>
						{footer}
					</Dialog.Content>
				</Dialog.Overlay>
			</Dialog.Portal>
		</Dialog.Root>
	);
}
