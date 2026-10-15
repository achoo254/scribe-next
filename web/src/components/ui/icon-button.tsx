import type { ComponentProps, ReactNode } from "react";
import { cn } from "../../lib/cn";
import {
	ButtonSpinner,
	type ButtonVariant,
	buttonClassName,
	pendingButtonProps,
} from "./button";

export type IconButtonSize = "sm" | "md";

export interface IconButtonProps
	extends Omit<ComponentProps<"button">, "children" | "aria-label"> {
	/** Accessible name; icon-only buttons have no visible text. */
	label: string;
	/** Tabler icon element, e.g. `<IconX stroke={1.5} />`. */
	icon: ReactNode;
	variant?: ButtonVariant;
	size?: IconButtonSize;
	/** Spinner in place of the icon; inert and `aria-busy` meanwhile. */
	loading?: boolean;
}

/** 32/36px squares; 40/44px on touch screens so the tap target stays large. */
const SQUARE: Record<IconButtonSize, string> = {
	sm: "size-8 pointer-coarse:size-10",
	md: "size-9 pointer-coarse:size-11",
};

const SPINNER_SIZE: Record<IconButtonSize, number> = { sm: 16, md: 20 };

export function IconButton({
	label,
	icon,
	variant = "ghost",
	size = "md",
	loading = false,
	disabled,
	onClick,
	"aria-disabled": ariaDisabled,
	className,
	type = "button",
	...rest
}: IconButtonProps) {
	return (
		<button
			type={type}
			aria-label={label}
			data-variant={variant}
			{...pendingButtonProps({ loading, disabled, ariaDisabled, onClick })}
			className={cn(
				buttonClassName({ variant, size, iconOnly: true }),
				SQUARE[size],
				loading && "relative",
				className,
			)}
			{...rest}
		>
			{loading ? <ButtonSpinner size={SPINNER_SIZE[size]} /> : icon}
		</button>
	);
}
