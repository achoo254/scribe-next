import { cn } from "../../lib/cn";
import { buttonClassName } from "./button";

export interface SegmentedOption<T extends string> {
	value: T;
	label: string;
	"data-id": string;
}

export interface SegmentedControlProps<T extends string> {
	/** Screen-reader name of the group (sr-only legend). */
	legend: string;
	options: readonly SegmentedOption<T>[];
	value: T;
	onChange: (value: T) => void;
	"data-id": string;
}

/**
 * One choice among a few views of the same content (code language,
 * config format): a fieldset of `aria-pressed` buttons, not tabs, since
 * the options share one output. Wraps onto a new line when space runs out.
 */
export function SegmentedControl<T extends string>({
	legend,
	options,
	value,
	onChange,
	"data-id": dataId,
}: SegmentedControlProps<T>) {
	return (
		<fieldset
			data-id={dataId}
			className="m-0 flex flex-wrap gap-2 border-0 p-0"
		>
			<legend className="sr-only">{legend}</legend>
			{options.map((option) => (
				<button
					key={option.value}
					type="button"
					aria-pressed={value === option.value}
					data-id={option["data-id"]}
					className={cn(
						buttonClassName({
							variant: value === option.value ? "primary" : "secondary",
							size: "sm",
						}),
						"font-mono pointer-coarse:min-h-11",
					)}
					onClick={() => onChange(option.value)}
				>
					{option.label}
				</button>
			))}
		</fieldset>
	);
}
