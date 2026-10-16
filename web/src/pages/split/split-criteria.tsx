import { FormField } from "../../components/ui/form-field";
import { Input } from "../../components/ui/input";
import { Select } from "../../components/ui/select";
import type { JobCriterion } from "../../features/split/split-job";
import { useT } from "../../i18n/i18n-context";
import type { MessageKey } from "../../i18n/message-types";
import { parsePositiveNumber } from "../../lib/format";

const CRITERIA = [
	{ value: "minutes", label: "split.criteria.byMinutes" },
	{ value: "megabytes", label: "split.criteria.byMegabytes" },
	{ value: "parts", label: "split.criteria.byParts" },
] as const satisfies ReadonlyArray<{ value: JobCriterion; label: MessageKey }>;

const VALUE_LABEL = {
	minutes: "split.criteria.minutesValue",
	megabytes: "split.criteria.megabytesValue",
	parts: "split.criteria.partsValue",
} as const satisfies Record<JobCriterion, MessageKey>;

function isCriterion(value: string): value is JobCriterion {
	return CRITERIA.some((option) => option.value === value);
}

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
	const t = useT();
	const invalid = parseSplitValue(criterion, value) === null;
	return (
		<div className="flex flex-wrap items-start gap-4">
			<FormField
				label={t("split.criteria.label")}
				className="w-full max-w-60"
				data-id="split-criterion"
			>
				<Select
					value={criterion}
					disabled={disabled}
					onChange={(event) => {
						const next = event.target.value;
						if (isCriterion(next)) onCriterionChange(next);
					}}
				>
					{CRITERIA.map((option) => (
						<option key={option.value} value={option.value}>
							{t(option.label)}
						</option>
					))}
				</Select>
			</FormField>
			<FormField
				label={t(VALUE_LABEL[criterion])}
				error={
					invalid
						? criterion === "parts"
							? t("split.criteria.partsInvalid")
							: t("split.criteria.valueInvalid")
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
