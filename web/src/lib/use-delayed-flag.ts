import { useEffect, useLayoutEffect, useRef, useState } from "react";

export interface DelayedFlagOptions {
	/** Milliseconds `active` must last before the flag turns on. */
	delay?: number;
	/** Once on, the flag stays on at least this many milliseconds. */
	minDuration?: number;
}

/** Wait before a loading indicator appears; loads faster than this show nothing. */
export const LOADING_DELAY_MS = 150;
/** Shortest time an indicator stays once shown, so it never blinks. */
export const LOADING_MIN_MS = 300;

/**
 * `active` filtered for loading indicators: true only once `active` has
 * lasted longer than `delay`, then held for at least `minDuration` even if
 * `active` ends sooner. Toggling faster than `delay` never turns it on, and
 * turning `active` back on while the flag is held keeps it on without a gap.
 */
export function useDelayedFlag(
	active: boolean,
	{
		delay = LOADING_DELAY_MS,
		minDuration = LOADING_MIN_MS,
	}: DelayedFlagOptions = {},
): boolean {
	const [shown, setShown] = useState(false);
	const shownAt = useRef(0);
	// Synced at commit, before any timer can run: on a slow machine the delay
	// timer can fire after `active` has ended but before the passive effect
	// below clears it, which would show the indicator over content that is
	// already on screen (and remount that content when it hides again).
	const activeNow = useRef(active);
	useLayoutEffect(() => {
		activeNow.current = active;
	}, [active]);

	useEffect(() => {
		if (active) {
			if (shown) return;
			const timer = window.setTimeout(() => {
				if (!activeNow.current) return;
				shownAt.current = Date.now();
				setShown(true);
			}, delay);
			return () => window.clearTimeout(timer);
		}
		if (!shown) return;
		const remaining = shownAt.current + minDuration - Date.now();
		const timer = window.setTimeout(
			() => setShown(false),
			Math.max(0, remaining),
		);
		return () => window.clearTimeout(timer);
	}, [active, shown, delay, minDuration]);

	return shown;
}
