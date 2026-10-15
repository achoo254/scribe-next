import { useEffect, useRef, useSyncExternalStore } from "react";
import { Recorder, type RecorderState } from "./recorder";

export interface RecorderApi extends RecorderState {
	recorder: Recorder;
}

/**
 * One `Recorder` for the lifetime of the component; mic and screen are
 * released when it unmounts. The audio level is read separately with
 * `useRecorderLevel` so only the meter re-renders every frame.
 */
export function useRecorder(): RecorderApi {
	const ref = useRef<Recorder | null>(null);
	ref.current ??= new Recorder();
	const recorder = ref.current;
	const state = useSyncExternalStore(recorder.subscribe, recorder.getState);
	useEffect(() => () => recorder.dispose(), [recorder]);
	return { ...state, recorder };
}

/** Anything that publishes an audio level (browser or native recorder). */
export interface LevelSource {
	getLevel: () => number;
	subscribeLevel: (listener: () => void) => () => void;
}

/** RMS level 0..1 of the recording's audio, updated every frame. */
export function useRecorderLevel(source: LevelSource): number {
	return useSyncExternalStore(source.subscribeLevel, source.getLevel);
}
