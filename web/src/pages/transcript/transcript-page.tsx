import { useCallback } from "react";
import { Navigate, useNavigate, useParams } from "react-router";
import { PATHS, transcriptPath } from "../../layout/app-paths";
import { TranscriptList } from "./transcript-list";
import { TranscriptReader } from "./transcript-reader";

/** `/transcripts`: every transcript; a tap opens `/transcripts/:key`. */
export function TranscriptListPage() {
	const navigate = useNavigate();
	return <TranscriptList onOpen={(key) => navigate(transcriptPath(key))} />;
}

/**
 * `/transcripts/:key`: one transcript. The router hands over the decoded
 * key (`s:<id>` arrives as `s%3A<id>`). Only routed for users who may
 * transcribe.
 */
export function TranscriptReaderPage() {
	const { key } = useParams();
	const navigate = useNavigate();
	const back = useCallback(
		(options?: { replace?: boolean }) => navigate(PATHS.transcripts, options),
		[navigate],
	);
	if (!key) return <Navigate to={PATHS.transcripts} replace />;
	return (
		<TranscriptReader
			key={key}
			transcriptKey={key}
			canTranscribe
			onBack={back}
		/>
	);
}
