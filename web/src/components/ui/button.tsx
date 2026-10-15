import { IconLoader2 } from "@tabler/icons-react";
import type { ComponentProps, MouseEvent, ReactNode } from "react";
import { cn } from "../../lib/cn";

export type ButtonVariant = "primary" | "secondary" | "ghost" | "danger";
export type ButtonSize = "sm" | "md" | "lg";

export interface ButtonStyleOptions {
	variant?: ButtonVariant;
	size?: ButtonSize;
	block?: boolean;
	/** Square icon-only control: keeps radius and font, drops min-height and padding. */
	iconOnly?: boolean;
}

const BASE =
	"inline-flex cursor-pointer select-none items-center justify-center gap-2 whitespace-nowrap border font-mono font-semibold leading-none " +
	"transition-[scale,background-color,border-color,box-shadow,color,opacity] " +
	"motion-safe:enabled:not-aria-disabled:hover:scale-103 motion-safe:enabled:not-aria-disabled:active:scale-97 " +
	"disabled:cursor-not-allowed disabled:opacity-45 disabled:shadow-none aria-busy:cursor-progress " +
	"[&_svg]:shrink-0";

const VARIANTS: Record<ButtonVariant, string> = {
	primary:
		"border-transparent bg-primary-solid text-on-primary shadow-glow-primary enabled:not-aria-disabled:hover:bg-primary-solid-hover",
	secondary:
		"border-border bg-surface text-fg enabled:not-aria-disabled:hover:border-[color-mix(in_srgb,var(--c-primary)_55%,var(--c-border))]",
	ghost:
		"border-transparent bg-transparent text-muted enabled:not-aria-disabled:hover:bg-fg/6 enabled:not-aria-disabled:hover:text-fg",
	danger:
		"border-transparent bg-error text-on-tone shadow-glow-error enabled:not-aria-disabled:hover:bg-[color-mix(in_srgb,var(--c-error)_88%,var(--c-text))]",
};

const SIZES: Record<ButtonSize, string> = {
	sm: "rounded-sm text-[0.8125rem]",
	md: "rounded-md text-sm",
	lg: "rounded-md text-base",
};

/** Height and padding of text buttons; icon-only buttons size themselves. */
const BOX: Record<ButtonSize, string> = {
	sm: "min-h-8 px-3 pointer-coarse:min-h-10",
	md: "min-h-10 px-4 pointer-coarse:min-h-11",
	/** Page-level calls to action (landing hero and closing band). */
	lg: "min-h-12 px-6 pointer-coarse:min-h-12",
};

/** Class list shared by Button, IconButton and link-styled buttons. */
export function buttonClassName({
	variant = "secondary",
	size = "md",
	block = false,
	iconOnly = false,
}: ButtonStyleOptions = {}): string {
	return cn(
		BASE,
		VARIANTS[variant],
		SIZES[size],
		!iconOnly && BOX[size],
		block && "w-full",
	);
}

export interface ButtonProps
	extends ComponentProps<"button">,
		ButtonStyleOptions {
	/** Tabler icon (or any node) rendered before the label. */
	leadingIcon?: ReactNode;
	/**
	 * The action this button started is running: a spinner covers the
	 * label (kept invisible so the width does not change), and the button
	 * is `aria-busy` and inert until it ends (see `pendingButtonProps`).
	 */
	loading?: boolean;
}

interface PendingInput {
	loading: boolean;
	disabled: boolean | undefined;
	ariaDisabled: ComponentProps<"button">["aria-disabled"];
	onClick: ComponentProps<"button">["onClick"];
}

/** A loading button's clicks do nothing, including submitting its form. */
function ignorePendingClick(event: MouseEvent<HTMLButtonElement>): void {
	event.preventDefault();
}

/**
 * State attributes shared by `Button` and `IconButton`. A loading button
 * is not `disabled`: that would drop focus from the button just pressed
 * and fade it. It is `aria-disabled` and swallows clicks instead, so
 * neither a second click nor Enter in its form submits again (implicit
 * submission clicks the default button, and the click is cancelled).
 */
export function pendingButtonProps({
	loading,
	disabled,
	ariaDisabled,
	onClick,
}: PendingInput) {
	return {
		disabled,
		"aria-disabled": loading ? true : ariaDisabled,
		"aria-busy": loading || undefined,
		onClick: loading ? ignorePendingClick : onClick,
	};
}

const SPINNER_SIZE: Record<ButtonSize, number> = { sm: 16, md: 18, lg: 20 };

/** Spinner of a loading button, centred over the hidden content. */
export function ButtonSpinner({ size }: { size: number }) {
	return (
		<span
			aria-hidden="true"
			data-id="button-spinner"
			className="absolute inset-0 flex items-center justify-center"
		>
			<IconLoader2 size={size} className="motion-safe:animate-spin" />
		</span>
	);
}

export function Button({
	variant = "secondary",
	size = "md",
	block = false,
	iconOnly = false,
	leadingIcon,
	loading = false,
	disabled,
	onClick,
	"aria-disabled": ariaDisabled,
	className,
	type = "button",
	children,
	...rest
}: ButtonProps) {
	return (
		<button
			type={type}
			data-variant={variant}
			data-size={size}
			{...pendingButtonProps({ loading, disabled, ariaDisabled, onClick })}
			className={cn(
				buttonClassName({ variant, size, block, iconOnly }),
				loading && "relative",
				className,
			)}
			{...rest}
		>
			{loading ? (
				<>
					{/* Same flex layout as the label, only invisible: the width
					    stays put and the accessible name is still read. */}
					<span className="inline-flex items-center gap-2 opacity-0">
						{leadingIcon}
						{children}
					</span>
					<ButtonSpinner size={SPINNER_SIZE[size]} />
				</>
			) : (
				<>
					{leadingIcon}
					{children}
				</>
			)}
		</button>
	);
}
