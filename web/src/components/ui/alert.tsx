import {
	IconAlertCircle,
	IconAlertTriangle,
	IconCircleCheck,
	IconInfoCircle,
} from "@tabler/icons-react";
import type { ComponentProps, ReactNode } from "react";
import { cn } from "../../lib/cn";

export type AlertTone = "info" | "success" | "warning" | "danger";

const TONES: Record<AlertTone, { tone: string; Icon: typeof IconInfoCircle }> =
	{
		info: { tone: "[--tone:var(--c-info)]", Icon: IconInfoCircle },
		success: { tone: "[--tone:var(--c-success)]", Icon: IconCircleCheck },
		warning: { tone: "[--tone:var(--c-warning)]", Icon: IconAlertTriangle },
		danger: { tone: "[--tone:var(--c-error)]", Icon: IconAlertCircle },
	};

export interface AlertProps extends Omit<ComponentProps<"div">, "title"> {
	tone?: AlertTone;
	title?: ReactNode;
}

/**
 * Inline message on a soft tone tint. Warning/danger announce assertively
 * (role="alert"); info/success politely (role="status").
 */
export function Alert({
	tone = "info",
	title,
	className,
	children,
	...rest
}: AlertProps) {
	const { tone: toneClass, Icon } = TONES[tone];
	const assertive = tone === "warning" || tone === "danger";
	return (
		<div
			role={assertive ? "alert" : "status"}
			data-tone={tone}
			className={cn(
				"tint-soft flex gap-3 rounded-md border px-4 py-3 text-fg text-sm [--tone-border-mix:35%]",
				toneClass,
				className,
			)}
			{...rest}
		>
			<Icon size={20} aria-hidden="true" className="text-(--tone)" />
			<div className="min-w-0 flex-1">
				{title ? (
					<strong className="mb-0.5 block font-mono text-sm">{title}</strong>
				) : null}
				{children}
			</div>
		</div>
	);
}
