import { useQueryClient } from "@tanstack/react-query";
import {
	type ReactNode,
	createContext,
	useCallback,
	useContext,
	useEffect,
	useRef,
	useState,
} from "react";
import { useAuth } from "../../auth/auth-context";
import { refreshStorage } from "../../features/files/file-queries";
import { userKeyOf } from "../../features/recorder/native-recorder";
import type { Recorder } from "../../features/recorder/recorder";
import type { RecordSources } from "../../features/recorder/recorder-messages";
import {
	type NativeRecorderApi,
	useNativeRecorder,
} from "../../features/recorder/use-native-recorder";
import {
	type RecorderApi,
	useRecorder,
} from "../../features/recorder/use-recorder";
import {
	type SplitJobApi,
	useSplitJob,
} from "../../features/split/use-split-job";
import type {
	BookmarkQueue,
	BookmarkQueueState,
} from "../../features/transcript-docs/bookmark-queue";
import { transcriptKey } from "../../features/transcript-docs/transcript-docs-api";
import { useBookmarkQueue } from "../../features/transcript-docs/use-bookmark-queue";
import { isNativeShell } from "../../lib/api";
import { parsePositiveNumber } from "../../lib/format";

/*
 * The recorder lives here, above the routes and below `RequireAuth`, not in
 * the record page: leaving /record keeps the recording, its clock, wake
 * lock and uploads going, and coming back shows the same session. Signing
 * out unmounts it, which releases the browser's microphone.
 */

/** Resolves once the recorder is no longer saving a recording. */
function saved(recorder: Recorder): Promise<void> {
	return new Promise((resolve) => {
		const unsubscribe = recorder.subscribe(() => {
			if (recorder.getState().finalizing) return;
			unsubscribe();
			resolve();
		});
		if (!recorder.getState().finalizing) {
			unsubscribe();
			resolve();
		}
	});
}

/** Bookmark at the recording clock of the browser recording being made. */
function markBrowser(rec: RecorderApi, queue: BookmarkQueue): void {
	const { recording, fileId } = rec.recorder.getState();
	if (!recording || !fileId) return;
	queue.mark(transcriptKey({ fileId }), rec.recorder.position());
}

/**
 * Bookmark at the session position Swift reports when pressed (the page's
 * own clock is stale after the phone was locked).
 */
async function markNative(
	rec: NativeRecorderApi,
	queue: BookmarkQueue,
	sessionId: string,
): Promise<void> {
	const at = await rec.recorder.position();
	queue.mark(transcriptKey({ session: sessionId }), at);
}

export interface BrowserRecordState {
	rec: RecorderApi;
	/** The cut after a recording; independent of the "Cắt file" page's job. */
	split: SplitJobApi;
	sources: RecordSources;
	setSources: (sources: RecordSources) => void;
	splitEnabled: boolean;
	setSplitEnabled: (enabled: boolean) => void;
	splitMinutes: string;
	setSplitMinutes: (minutes: string) => void;
	/** Starts, or stops and saves (then cuts, when asked to). */
	toggle: () => Promise<void>;
	/**
	 * Stops and waits until the recording is saved: sign-out calls it
	 * first, so no chunk is sent after the session is gone.
	 */
	stopAndSave: () => Promise<void>;
	/** Bookmarks of the current recording. */
	bookmarks: BookmarkQueueState;
	/** Null when this recording cannot take bookmarks (backend gave no file id). */
	mark: (() => void) | null;
}

export type SegmentMinutes = "1" | "5" | "10";

export interface NativeRecordState {
	rec: NativeRecorderApi;
	/** Segment length chosen for the next recording. */
	minutes: SegmentMinutes;
	setMinutes: (minutes: SegmentMinutes) => void;
	toggle: () => Promise<void>;
	bookmarks: BookmarkQueueState;
	/** Null while no session is known yet. */
	mark: (() => void) | null;
}

const BrowserRecordContext = createContext<BrowserRecordState | null>(null);
const NativeRecordContext = createContext<NativeRecordState | null>(null);

export function BrowserRecordProvider({ children }: { children: ReactNode }) {
	const queryClient = useQueryClient();
	const rec = useRecorder();
	const split = useSplitJob();
	const { queue, ...bookmarks } = useBookmarkQueue();
	const { recorder } = rec;
	const [sources, setSources] = useState<RecordSources>({
		mic: true,
		systemAudio: false,
		screen: false,
	});
	const [splitEnabled, setSplitEnabled] = useState(false);
	const [splitMinutes, setSplitMinutes] = useState("10");

	// A finished cut (done: segments replace the recording; failed: the
	// original is still there) changes the history and the quota.
	useEffect(() => {
		if (split.status === "done" || split.status === "failed") {
			void refreshStorage(queryClient, ["recordings"]);
		}
	}, [split.status, queryClient]);

	// A recording that stopped by itself (share ended, storage full) was
	// saved without `toggle`: show it in the history too. `toggle` refreshes
	// on its own, after the optional cut.
	const toggling = useRef(false);
	const wasFinalizing = useRef(false);
	useEffect(() => {
		if (wasFinalizing.current && !rec.finalizing) {
			if (toggling.current) toggling.current = false;
			else void refreshStorage(queryClient, ["recordings"]);
		}
		wasFinalizing.current = rec.finalizing;
	}, [rec.finalizing, queryClient]);

	const stopAndSave = useCallback(async () => {
		await recorder.stop();
		await saved(recorder);
	}, [recorder]);

	async function toggle(): Promise<void> {
		if (!recorder.getState().recording) {
			split.reset();
			queue.reset();
			await recorder.start(sources);
			return;
		}
		const minutes = parsePositiveNumber(splitMinutes);
		// Cleared by the effect above once this stop has finished saving.
		toggling.current = true;
		const result = await recorder.stop();
		if (!result) {
			// Nothing saved by this stop: the recording had already stopped by
			// itself (storage full) and saved on its own, possibly while this
			// flag kept the effect above from refreshing. Show it now.
			toggling.current = false;
			void refreshStorage(queryClient, ["recordings"]);
			return;
		}
		if (splitEnabled && minutes !== null) {
			// The history refreshes when the job ends, so the original does
			// not flash in before being replaced by its segments.
			void refreshStorage(queryClient);
			await split.start({
				inputFileId: result.fileId,
				criterion: "minutes",
				value: minutes,
				recordingSplit: true,
			});
		} else {
			void refreshStorage(queryClient, ["recordings"]);
		}
	}

	return (
		<BrowserRecordContext.Provider
			value={{
				rec,
				split,
				sources,
				setSources,
				splitEnabled,
				setSplitEnabled,
				splitMinutes,
				setSplitMinutes,
				toggle,
				stopAndSave,
				bookmarks,
				mark:
					rec.recording && rec.fileId ? () => markBrowser(rec, queue) : null,
			}}
		>
			{children}
		</BrowserRecordContext.Provider>
	);
}

export function NativeRecordProvider({ children }: { children: ReactNode }) {
	// Uploads of this account resume once, when the provider mounts for
	// the signed-in session (see `useNativeRecorder`), not per page visit.
	const rec = useNativeRecorder();
	const { user } = useAuth();
	const [minutes, setMinutes] = useState<SegmentMinutes>("1");
	const { queue, ...bookmarks } = useBookmarkQueue();

	async function toggle(): Promise<void> {
		if (rec.recorder.getState().recording) {
			await rec.recorder.stop();
			return;
		}
		if (!user) return;
		const key = rec.userKey ?? (await userKeyOf(user.email));
		queue.reset();
		await rec.recorder.start(key, Number(minutes));
	}

	const { sessionId } = rec;
	const mark =
		rec.recording && sessionId
			? () => void markNative(rec, queue, sessionId)
			: null;

	return (
		<NativeRecordContext.Provider
			value={{ rec, minutes, setMinutes, toggle, bookmarks, mark }}
		>
			{children}
		</NativeRecordContext.Provider>
	);
}

/** The recorder of this platform: Swift in the iOS app, else the browser's. */
export function RecordStateProvider({ children }: { children: ReactNode }) {
	return isNativeShell() ? (
		<NativeRecordProvider>{children}</NativeRecordProvider>
	) : (
		<BrowserRecordProvider>{children}</BrowserRecordProvider>
	);
}

/** Browser recording state, or null outside its provider (iOS app, tests). */
export function useOptionalBrowserRecordState(): BrowserRecordState | null {
	return useContext(BrowserRecordContext);
}

/** iOS recording state, or null outside its provider (browser, tests). */
export function useOptionalNativeRecordState(): NativeRecordState | null {
	return useContext(NativeRecordContext);
}

export function useBrowserRecordState(): BrowserRecordState {
	const state = useContext(BrowserRecordContext);
	if (!state)
		throw new Error("useBrowserRecordState needs <RecordStateProvider>");
	return state;
}

export function useNativeRecordState(): NativeRecordState {
	const state = useContext(NativeRecordContext);
	if (!state)
		throw new Error("useNativeRecordState needs <RecordStateProvider>");
	return state;
}
