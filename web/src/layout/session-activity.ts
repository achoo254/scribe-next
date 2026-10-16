import {
	createContext,
	useCallback,
	useContext,
	useEffect,
	useMemo,
	useState,
} from "react";

/**
 * Work that signing out would interrupt. The recorders and the file upload
 * live inside their pages, so each page reports its own part here and the
 * account menu reads the sum to ask before signing out.
 */
export interface SessionActivity {
	/** A recording is starting, running or being saved. */
	recording: boolean;
	/** iOS app: recorded segments kept on the phone, not uploaded yet. */
	segmentsWaiting: number;
	/** "Cắt file": a chosen file is still uploading. */
	fileUploading: boolean;
}

export interface ActivityReport extends SessionActivity {
	/**
	 * Stops and saves the browser recording. Called before the session is
	 * deleted, so the last chunks still reach the account they belong to.
	 */
	stopRecording?: () => Promise<unknown>;
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

type Publish = (source: string, report: ActivityReport | null) => void;

const PublishActivityContext = createContext<Publish | null>(null);

export const SessionActivityProvider = PublishActivityContext.Provider;

function sameReport(a: ActivityReport | undefined, b: ActivityReport): boolean {
	return (
		a !== undefined &&
		a.recording === b.recording &&
		a.segmentsWaiting === b.segmentsWaiting &&
		a.fileUploading === b.fileUploading &&
		a.stopRecording === b.stopRecording
	);
}

export interface SessionActivityState {
	activity: SessionActivity;
	/** Stops every running browser recording (each one saves itself). */
	stopRecordings: () => Promise<void>;
	publish: Publish;
}

/** The shell's store of what each page reported. */
export function useSessionActivityState(): SessionActivityState {
	const [reports, setReports] = useState<
		Readonly<Record<string, ActivityReport>>
	>({});

	const publish = useCallback<Publish>((source, report) => {
		setReports((current) => {
			if (report === null) {
				if (!(source in current)) return current;
				const { [source]: _removed, ...rest } = current;
				return rest;
			}
			return sameReport(current[source], report)
				? current
				: { ...current, [source]: report };
		});
	}, []);

	return useMemo(() => {
		const all = Object.values(reports);
		const activity: SessionActivity = {
			recording: all.some((report) => report.recording),
			segmentsWaiting: all.reduce(
				(sum, report) => sum + report.segmentsWaiting,
				0,
			),
			fileUploading: all.some((report) => report.fileUploading),
		};
		const stopRecordings = async (): Promise<void> => {
			const stops = all.flatMap((report) =>
				report.recording && report.stopRecording ? [report.stopRecording] : [],
			);
			// Sign-out goes on even if saving fails: the user asked to leave.
			await Promise.allSettled(stops.map((stop) => stop()));
		};
		return { activity, stopRecordings, publish };
	}, [reports, publish]);
}

/**
 * Reports a page's part of the session activity while it is mounted.
 * Outside the app shell (unit tests of a page) it does nothing.
 */
export function useReportSessionActivity(
	source: string,
	report: ActivityReport,
): void {
	const publish = useContext(PublishActivityContext);
	const { recording, segmentsWaiting, fileUploading, stopRecording } = report;
	useEffect(() => {
		publish?.(source, {
			recording,
			segmentsWaiting,
			fileUploading,
			stopRecording,
		});
	}, [
		publish,
		source,
		recording,
		segmentsWaiting,
		fileUploading,
		stopRecording,
	]);
	useEffect(() => () => publish?.(source, null), [publish, source]);
}
