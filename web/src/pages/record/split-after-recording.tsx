import { FormField } from "../../components/ui/form-field";
import { Input } from "../../components/ui/input";
import { SwitchField } from "../../components/ui/switch-field";
import { parsePositiveNumber } from "../../lib/format";

export interface SplitAfterRecordingProps {
	enabled: boolean;
	onEnabledChange: (enabled: boolean) => void;
	minutes: string;
	onMinutesChange: (minutes: string) => void;
	disabled: boolean;
}

/** Optional: cut the finished recording into N-minute recordings. */
export function SplitAfterRecording({
	enabled,
	onEnabledChange,
	minutes,
	onMinutesChange,
	disabled,
}: SplitAfterRecordingProps) {
	const invalid = enabled && parsePositiveNumber(minutes) === null;
	return (
		<div
			data-id="record-split"
			className="flex flex-wrap items-start gap-x-6 gap-y-3"
		>
			<SwitchField
				label="Cắt thành đoạn theo phút"
				checked={enabled}
				disabled={disabled}
				data-id="record-split-toggle"
				onCheckedChange={onEnabledChange}
			/>
			{enabled ? (
				<FormField
					label="Số phút mỗi đoạn"
					error={invalid ? "Nhập số phút lớn hơn 0." : undefined}
					className="w-full max-w-50"
					data-id="record-split-minutes"
				>
					<Input
						type="number"
						inputMode="decimal"
						min={0}
						step="any"
						value={minutes}
						disabled={disabled}
						onChange={(event) => onMinutesChange(event.target.value)}
					/>
				</FormField>
			) : null}
		</div>
	);
}
