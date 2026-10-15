import type { ReactNode } from "react";
import { cn } from "../../lib/cn";
import { cardClassName } from "./card";

export interface EmptyStateProps {
	icon: ReactNode;
	title: ReactNode;
	/** Page-level states (403) use h1; sections inside a page use h2. */
	headingLevel?: 1 | 2;
	children?: ReactNode;
	action?: ReactNode;
	"data-id"?: string;
}

/** Centred card with an icon tile, a title, a short text and one action. */
export function EmptyState({
	icon,
	title,
	headingLevel = 2,
	children,
	action,
	"data-id": dataId,
}: EmptyStateProps) {
	const Heading = headingLevel === 1 ? "h1" : "h2";
	return (
		<div
			data-id={dataId}
			className={cn(
				cardClassName,
				"grid justify-items-center gap-3 px-6 py-12 text-center",
			)}
		>
			<span className="grid size-16 place-items-center rounded-lg border border-border border-dashed bg-surface-subtle text-icon-muted">
				{icon}
			</span>
			<Heading className="m-0 font-bold font-mono text-lg">{title}</Heading>
			{children ? (
				<p className="m-0 max-w-90 text-muted text-sm">{children}</p>
			) : null}
			{action}
		</div>
	);
}
