import { useEffect, useRef, useSyncExternalStore } from "react";
import { LiveTranscript, type LiveTranscriptState } from "./live-transcript";
import { NativeRecorderPlugin } from "./native-recorder";

export interface LiveTranscriptApi extends LiveTranscriptState {
	transcript: LiveTranscript;
}

/**
 * The live transcript Swift keeps, bound to React: listens to its events
 * and replaces the state with `getTranscript()` whenever the page becomes
 * visible again (the WebView was suspended while the phone was locked).
 */
export function useLiveTranscript(): LiveTranscriptApi {
	const ref = useRef<LiveTranscript | null>(null);
	ref.current ??= new LiveTranscript(NativeRecorderPlugin);
	const transcript = ref.current;
	const state = useSyncExternalStore(transcript.subscribe, transcript.getState);

	useEffect(() => {
		void transcript.attach();
		const onVisible = () => {
			if (document.visibilityState === "visible") void transcript.refresh();
		};
		document.addEventListener("visibilitychange", onVisible);
		return () => {
			document.removeEventListener("visibilitychange", onVisible);
			transcript.dispose();
		};
	}, [transcript]);

	return { ...state, transcript };
}
