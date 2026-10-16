import { useCallback } from "react";
import { useNavigate } from "react-router";
import { useAuth } from "../auth/auth-context";
import { transcriptPath } from "./app-paths";

/**
 * Opens a transcript (navigates to `/transcripts/:key`); null when the
 * user may not transcribe, since those routes do not exist for them.
 */
export function useOpenTranscript(): ((key: string) => void) | null {
	const { user } = useAuth();
	const navigate = useNavigate();
	const open = useCallback(
		(key: string) => navigate(transcriptPath(key)),
		[navigate],
	);
	return user?.canTranscribe ? open : null;
}
