import { Children, type ReactNode } from "react";
import { cn } from "../../lib/cn";

export interface ModalFooterProps {
	/** One or two actions; two render as equal columns (cancel, confirm). */
	children: ReactNode;
	className?: string;
}

/** Sticky modal footer: two equal columns, or one when a single action. */
export function ModalFooter({ children, className }: ModalFooterProps) {
	const single = Children.toArray(children).length === 1;
	return (
		<div
			data-layout={single ? "single" : "split"}
			className={cn(
				"sticky bottom-0 grid gap-3 border-border-subtle border-t bg-surface-subtle px-6 py-3",
				single ? "grid-cols-1" : "grid-cols-2",
				className,
			)}
		>
			{children}
		</div>
	);
}
