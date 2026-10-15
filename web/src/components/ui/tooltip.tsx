import * as RadixTooltip from "@radix-ui/react-tooltip";
import type { ReactNode } from "react";

/** Mount once near the app root; shares open/close delays across tooltips. */
export function TooltipProvider({ children }: { children: ReactNode }) {
	return (
		<RadixTooltip.Provider delayDuration={300} skipDelayDuration={150}>
			{children}
		</RadixTooltip.Provider>
	);
}

export interface TooltipProps {
	content: ReactNode;
	/** A single focusable element (rendered via Radix `asChild`). */
	children: ReactNode;
	side?: "top" | "right" | "bottom" | "left";
}

/** Inverted label (text colour on surface colour), as in the prototype. */
export function Tooltip({ content, children, side = "top" }: TooltipProps) {
	return (
		<RadixTooltip.Root>
			<RadixTooltip.Trigger asChild>{children}</RadixTooltip.Trigger>
			<RadixTooltip.Portal>
				<RadixTooltip.Content
					side={side}
					sideOffset={8}
					collisionPadding={8}
					className="z-150 max-w-60 rounded-sm bg-fg px-2.5 py-1.5 font-medium font-sans text-surface text-xs leading-[1.45] shadow-md data-[state=closed]:animate-fade-out data-[state=delayed-open]:animate-pop-in data-[state=instant-open]:animate-pop-in"
				>
					{content}
				</RadixTooltip.Content>
			</RadixTooltip.Portal>
		</RadixTooltip.Root>
	);
}
