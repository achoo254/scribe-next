import {
	useOptionalBrowserRecordState,
	useOptionalNativeRecordState,
} from "../pages/record/record-state";
import { useOptionalSplitPageState } from "../pages/split/split-state";

/**
 * Work that signing out would interrupt, read from the app-level state
 * providers (record and "Cắt file"), so the account menu can ask first.
 */
export interface SessionActivity {
	/** A recording is starting, running or being saved. */
	recording: boolean;
	/** iOS app: recorded segments kept on the phone, not uploaded yet. */
	segmentsWaiting: number;
	/** "Cắt file": a chosen file is still uploading. */
	fileUploading: boolean;
}

export const IDLE_ACTIVITY: SessionActivity = {
	recording: false,
	segmentsWaiting: 0,
	fileUploading: false,
};

export function isSessionBusy(activity: SessionActivity): boolean {
	return (
		activity.recording || activity.segmentsWaiting > 0 || activity.fileUploading
	);
}

export interface SessionActivityState {
	activity: SessionActivity;
	/**
	 * Stops and saves the browser recording, so the last chunks reach the
	 * account they belong to before the session is deleted. The iOS
	 * recorder stops itself on sign-out (`Recorder.signOut`).
	 */
	stopRecordings: () => Promise<void>;
}

const NOTHING_TO_STOP = async (): Promise<void> => undefined;

/** What is running now; idle outside the providers (unit tests). */
export function useSessionActivity(): SessionActivityState {
	const browser = useOptionalBrowserRecordState();
	const native = useOptionalNativeRecordState();
	const split = useOptionalSplitPageState();
	const browserBusy = browser
		? browser.rec.recording || browser.rec.starting || browser.rec.finalizing
		: false;
	const nativeBusy = native
		? native.rec.recording || native.rec.starting || native.rec.finalizing
		: false;
	const stopAndSave = browser?.stopAndSave;
	return {
		activity: {
			recording: browserBusy || nativeBusy,
			segmentsWaiting: native?.rec.uploadsWaiting ?? 0,
			fileUploading: split?.upload.uploading ?? false,
		},
		stopRecordings:
			browserBusy && stopAndSave
				? async () => {
						// Sign-out goes on even if saving fails: the user asked to leave.
						await stopAndSave().catch(() => undefined);
					}
				: NOTHING_TO_STOP,
	};
}
