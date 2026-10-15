import { useCallback, useEffect, useRef, useState } from "react";
import {
	IDLE_JOB,
	SplitJob,
	type SplitJobState,
	type SplitRequest,
} from "./split-job";

export interface SplitJobApi extends SplitJobState {
	start: (request: SplitRequest) => Promise<void>;
	reset: () => void;
}

/** One split job bound to React state; polling stops on unmount. */
export function useSplitJob(): SplitJobApi {
	const [state, setState] = useState<SplitJobState>(IDLE_JOB);
	const job = useRef<SplitJob | null>(null);
	job.current ??= new SplitJob(setState);

	useEffect(() => () => job.current?.stop(), []);

	const start = useCallback(
		(request: SplitRequest) => job.current?.start(request) ?? Promise.resolve(),
		[],
	);
	const reset = useCallback(() => job.current?.reset(), []);

	return { ...state, start, reset };
}
