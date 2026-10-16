import type { ReactNode } from "react";
import { RecordStateProvider } from "../pages/record/record-state";
import { SplitPageProvider } from "../pages/split/split-state";

/**
 * State of the signed-in session that outlives a page: the recorder and
 * the "Cắt file" upload and job. Mounted under `RequireAuth`, so signing
 * out unmounts it (the browser's microphone is released) and the next
 * account starts clean.
 */
export function SignedInState({ children }: { children: ReactNode }) {
	return (
		<RecordStateProvider>
			<SplitPageProvider>{children}</SplitPageProvider>
		</RecordStateProvider>
	);
}
