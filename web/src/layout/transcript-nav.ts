import { createContext, useContext } from "react";

/**
 * Opens a transcript in the Transcript tab; provided by the app shell, null
 * when the tab is not available (the user may not transcribe).
 */
export const OpenTranscriptContext = createContext<
	((key: string) => void) | null
>(null);

export function useOpenTranscript(): ((key: string) => void) | null {
	return useContext(OpenTranscriptContext);
}
