/* Copied from localman-next apps/web/src/components/ui/textarea.tsx (same design
   system). Keep in sync with the source; change it here only when Scribe
   needs to, and say why. */
import type { ComponentProps } from "react";
import { cn } from "../../lib/cn";
import { useFormFieldControl } from "./form-field";
import { controlClassName } from "./input";

export interface TextareaProps extends ComponentProps<"textarea"> {
	invalid?: boolean;
}

export function Textarea({
	id,
	invalid,
	required,
	className,
	"aria-describedby": describedBy,
	...rest
}: TextareaProps) {
	const control = useFormFieldControl({
		id,
		invalid,
		required,
		"aria-describedby": describedBy,
	});
	return (
		<textarea
			{...control}
			className={cn(
				controlClassName,
				"min-h-24 resize-y p-3 leading-[1.55]",
				className,
			)}
			{...rest}
		/>
	);
}
