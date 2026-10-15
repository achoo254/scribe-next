import type { ComponentProps } from "react";
import { cn } from "../../lib/cn";
import { useFormFieldControl } from "./form-field";

/** Shared look for text-like controls (input, select, textarea). */
export const controlClassName =
	"w-full rounded-md border border-border-strong bg-surface font-mono text-sm text-fg " +
	"transition-[border-color,box-shadow] placeholder:text-muted placeholder:opacity-75 " +
	"focus:border-primary focus:shadow-focus-primary focus:outline-none " +
	"disabled:cursor-not-allowed disabled:bg-surface-subtle disabled:text-muted " +
	"aria-invalid:border-error aria-invalid:focus:shadow-focus-error";

export interface InputProps extends ComponentProps<"input"> {
	/** Marks the value invalid (red border, aria-invalid). */
	invalid?: boolean;
}

export function Input({
	id,
	invalid,
	required,
	className,
	"aria-describedby": describedBy,
	type = "text",
	...rest
}: InputProps) {
	const control = useFormFieldControl({
		id,
		invalid,
		required,
		"aria-describedby": describedBy,
	});
	return (
		<input
			type={type}
			{...control}
			className={cn(controlClassName, "min-h-11 px-3", className)}
			{...rest}
		/>
	);
}
