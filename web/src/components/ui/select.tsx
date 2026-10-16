/* Copied from localman-next apps/web/src/components/ui/select.tsx (same design
   system). Keep in sync with the source; change it here only when Scribe
   needs to, and say why. */
import { IconChevronDown } from "@tabler/icons-react";
import type { ComponentProps } from "react";
import { cn } from "../../lib/cn";
import { useFormFieldControl } from "./form-field";
import { controlClassName } from "./input";

export interface SelectProps extends ComponentProps<"select"> {
	invalid?: boolean;
}

/** Native <select> styled like Input; keeps platform a11y and mobile pickers. */
export function Select({
	id,
	invalid,
	required,
	className,
	"aria-describedby": describedBy,
	children,
	...rest
}: SelectProps) {
	const control = useFormFieldControl({
		id,
		invalid,
		required,
		"aria-describedby": describedBy,
	});
	return (
		<div className="relative">
			<select
				{...control}
				className={cn(
					controlClassName,
					"min-h-11 cursor-pointer appearance-none pr-9 pl-3",
					className,
				)}
				{...rest}
			>
				{children}
			</select>
			<IconChevronDown
				aria-hidden="true"
				size={18}
				className="-translate-y-1/2 pointer-events-none absolute top-1/2 right-2.5 text-icon-muted"
			/>
		</div>
	);
}
