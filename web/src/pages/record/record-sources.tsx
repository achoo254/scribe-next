import { SwitchField } from "../../components/ui/switch-field";
import type { RecordSources } from "../../features/recorder/recorder-messages";
import { useT } from "../../i18n/i18n-context";
import type { MessageKey } from "../../i18n/message-types";

const SOURCES = [
	{ key: "mic", label: "record.sources.mic" },
	{ key: "systemAudio", label: "record.sources.systemAudio" },
	{ key: "screen", label: "record.sources.screen" },
] as const satisfies ReadonlyArray<{
	key: keyof RecordSources;
	label: MessageKey;
}>;

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
	const t = useT();
	return (
		<fieldset data-id="record-sources" className="m-0 grid gap-1 border-0 p-0">
			<legend className="mb-1 p-0 font-mono font-semibold text-[0.8125rem] text-fg">
				{t("record.sources.legend")}
			</legend>
			{SOURCES.map(({ key, label }) => (
				<SwitchField
					key={key}
					label={t(label)}
					checked={value[key]}
					disabled={disabled}
					data-id={`record-source-${key}`}
					onCheckedChange={(checked) => onChange({ ...value, [key]: checked })}
				/>
			))}
		</fieldset>
	);
}
