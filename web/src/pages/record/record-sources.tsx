import { SwitchField } from "../../components/ui/switch-field";
import type { RecordSources } from "../../features/recorder/recorder-messages";

const SOURCES: ReadonlyArray<{ key: keyof RecordSources; label: string }> = [
	{ key: "mic", label: "Micro" },
	{ key: "systemAudio", label: "Tiếng hệ thống" },
	{ key: "screen", label: "Ghi kèm màn hình (video)" },
];

export interface RecordSourcesFieldProps {
	value: RecordSources;
	onChange: (next: RecordSources) => void;
	disabled: boolean;
}

/** Three independent sources; several are mixed into one recording. */
export function RecordSourcesField({
	value,
	onChange,
	disabled,
}: RecordSourcesFieldProps) {
	return (
		<fieldset data-id="record-sources" className="m-0 grid gap-1 border-0 p-0">
			<legend className="mb-1 p-0 font-mono font-semibold text-[0.8125rem] text-fg">
				Nguồn
			</legend>
			{SOURCES.map(({ key, label }) => (
				<SwitchField
					key={key}
					label={label}
					checked={value[key]}
					disabled={disabled}
					data-id={`record-source-${key}`}
					onCheckedChange={(checked) => onChange({ ...value, [key]: checked })}
				/>
			))}
		</fieldset>
	);
}
