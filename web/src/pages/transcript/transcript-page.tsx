import { TranscriptList } from "./transcript-list";
import { TranscriptReader } from "./transcript-reader";

export interface TranscriptPageProps {
	/** The tab is shown: lists and readers load and poll only then. */
	active: boolean;
	/** Transcript open in the reader; null shows the list. */
	openKey: string | null;
	onOpenKeyChange: (key: string | null) => void;
	canTranscribe: boolean;
}

/** Tab "Transcript": the list, or one transcript in the reader. */
export function TranscriptPage({
	active,
	openKey,
	onOpenKeyChange,
	canTranscribe,
}: TranscriptPageProps) {
	if (openKey) {
		return (
			<TranscriptReader
				key={openKey}
				transcriptKey={openKey}
				active={active}
				canTranscribe={canTranscribe}
				onBack={() => onOpenKeyChange(null)}
			/>
		);
	}
	return <TranscriptList active={active} onOpen={onOpenKeyChange} />;
}
