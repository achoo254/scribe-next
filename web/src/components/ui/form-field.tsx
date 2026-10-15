import {
	type ReactNode,
	createContext,
	useContext,
	useId,
	useMemo,
} from "react";
import { cn } from "../../lib/cn";

interface FormFieldContextValue {
	controlId: string;
	describedBy: string | undefined;
	invalid: boolean;
	required: boolean;
}

const FormFieldContext = createContext<FormFieldContextValue | null>(null);

export interface FormFieldControlInput {
	id?: string;
	invalid?: boolean;
	required?: boolean;
	"aria-describedby"?: string;
}

export interface FormFieldControlProps {
	id: string | undefined;
	required: boolean | undefined;
	"aria-invalid": true | undefined;
	"aria-describedby": string | undefined;
}

/**
 * Resolves id / aria wiring for a control. Explicit props win; otherwise the
 * enclosing FormField supplies them so label, hint and error stay linked.
 */
export function useFormFieldControl(
	props: FormFieldControlInput,
): FormFieldControlProps {
	const field = useContext(FormFieldContext);
	const invalid = props.invalid ?? field?.invalid ?? false;
	const describedBy =
		[props["aria-describedby"], field?.describedBy].filter(Boolean).join(" ") ||
		undefined;
	return {
		id: props.id ?? field?.controlId,
		required: props.required ?? (field?.required || undefined),
		"aria-invalid": invalid ? true : undefined,
		"aria-describedby": describedBy,
	};
}

export interface FormFieldProps {
	label: ReactNode;
	/** Single control (Input, Select, Textarea) to label. */
	children: ReactNode;
	hint?: ReactNode;
	/** Validation message; marks the control invalid when present. */
	error?: ReactNode;
	required?: boolean;
	/**
	 * Announce the error as soon as it appears (`role="alert"`), for errors
	 * that come back from a request rather than from typing.
	 */
	announceError?: boolean;
	/** Overrides the generated control id. */
	id?: string;
	className?: string;
	"data-id"?: string;
}

const TEXT_SMALL = "text-[0.8125rem] leading-normal";

export function FormField({
	label,
	children,
	hint,
	error,
	required = false,
	announceError = false,
	id,
	className,
	"data-id": dataId,
}: FormFieldProps) {
	const generatedId = useId();
	const controlId = id ?? `field-${generatedId}`;
	const hintId = hint ? `${controlId}-hint` : undefined;
	const errorId = error ? `${controlId}-error` : undefined;
	const invalid = Boolean(error);

	const value = useMemo<FormFieldContextValue>(
		() => ({
			controlId,
			describedBy: [hintId, errorId].filter(Boolean).join(" ") || undefined,
			invalid,
			required,
		}),
		[controlId, hintId, errorId, invalid, required],
	);

	return (
		<FormFieldContext.Provider value={value}>
			<div className={cn("grid gap-1.5", className)} data-id={dataId}>
				<label
					htmlFor={controlId}
					className="font-mono text-[0.8125rem] font-semibold text-fg"
				>
					{label}
					{required ? (
						<span aria-hidden="true" className="text-error">
							{" *"}
						</span>
					) : null}
				</label>
				{children}
				{hint ? (
					<p id={hintId} className={cn(TEXT_SMALL, "text-muted")}>
						{hint}
					</p>
				) : null}
				{error ? (
					<p
						id={errorId}
						role={announceError ? "alert" : undefined}
						className={cn(TEXT_SMALL, "text-error")}
					>
						{error}
					</p>
				) : null}
			</div>
		</FormFieldContext.Provider>
	);
}
