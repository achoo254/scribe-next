import { FormField } from "../../components/ui/form-field";
import { Input } from "../../components/ui/input";
import { SegmentedControl } from "../../components/ui/segmented-control";
import type { JobCriterion } from "../../features/split/split-job";
import { parsePositiveNumber } from "../../lib/format";

const OPTIONS = [
	{ value: "minutes", label: "Phút", "data-id": "split-criterion-minutes" },
	{ value: "megabytes", label: "MB", "data-id": "split-criterion-megabytes" },
	{ value: "parts", label: "Số phần", "data-id": "split-criterion-parts" },
] as const;

const VALUE_LABEL: Record<JobCriterion, string> = {
	minutes: "Số phút mỗi đoạn",
	megabytes: "MB mỗi đoạn",
	parts: "Chia thành (số đoạn)",
};

/** The value to send, or null when the input does not fit the criterion. */
export function parseSplitValue(
	criterion: JobCriterion,
	raw: string,
): number | null {
	return parsePositiveNumber(raw, { integer: criterion === "parts" });
}

export interface SplitCriteriaProps {
	criterion: JobCriterion;
	onCriterionChange: (criterion: JobCriterion) => void;
	value: string;
	onValueChange: (value: string) => void;
	disabled: boolean;
}

/** Cut by minutes, by megabytes, or into N equal parts. */
export function SplitCriteria({
	criterion,
	onCriterionChange,
	value,
	onValueChange,
	disabled,
}: SplitCriteriaProps) {
	return (
		<div className="grid gap-4">
			<div className="grid gap-1.5">
				<span
					aria-hidden="true"
					className="font-mono font-semibold text-[0.8125rem] text-fg"
				>
					Tiêu chí
				</span>
				<SegmentedControl
					legend="Tiêu chí"
					data-id="split-criterion"
					options={OPTIONS}
					value={criterion}
					onChange={(next) => {
						if (!disabled) onCriterionChange(next);
					}}
				/>
			</div>
			<FormField
				label={VALUE_LABEL[criterion]}
				error={
					parseSplitValue(criterion, value) === null
						? criterion === "parts"
							? "Nhập số nguyên, từ 1 trở lên."
							: "Nhập số lớn hơn 0."
						: undefined
				}
				className="w-full max-w-60"
				data-id="split-value"
			>
				<Input
					type="number"
					inputMode={criterion === "parts" ? "numeric" : "decimal"}
					min={criterion === "parts" ? 1 : 0}
					step={criterion === "parts" ? 1 : "any"}
					value={value}
					onChange={(event) => onValueChange(event.target.value)}
				/>
			</FormField>
		</div>
	);
}
