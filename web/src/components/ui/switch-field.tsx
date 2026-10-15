import { useId } from "react";
import { Switch } from "./switch";

export interface SwitchFieldProps {
	label: string;
	checked: boolean;
	onCheckedChange: (checked: boolean) => void;
	disabled?: boolean;
	"data-id"?: string;
}

/** Switch with its visible label; the row is at least 44px tall. */
export function SwitchField({
	label,
	checked,
	onCheckedChange,
	disabled = false,
	"data-id": dataId,
}: SwitchFieldProps) {
	const id = useId();
	return (
		<div className="flex min-h-11 items-center gap-3 has-disabled:opacity-60">
			<Switch
				id={id}
				className="peer"
				checked={checked}
				disabled={disabled}
				data-id={dataId}
				onCheckedChange={onCheckedChange}
			/>
			<label
				htmlFor={id}
				className="cursor-pointer text-sm peer-disabled:cursor-not-allowed"
			>
				{label}
			</label>
		</div>
	);
}
