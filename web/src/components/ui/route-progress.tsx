/* Copied from localman-next apps/web/src/components/ui/route-progress.tsx (same
   design system). Keep in sync with the source; change it here only when
   Scribe needs to, and say why. */
import { useEffect, useSyncExternalStore } from "react";
import { useNavigation } from "react-router";
import { useDelayedFlag } from "../../lib/use-delayed-flag";
import { ProgressBar } from "./progress-bar";

/*
 * Lazy page chunks still loading. Each Suspense fallback of a lazy route
 * mounts `RouteChunkPending`, which counts itself here until the chunk
 * arrives. A module store instead of a context: the fallbacks sit deep
 * inside the router while the bar lives in the layouts, and both layouts
 * would otherwise need a provider above the router.
 */
let pendingChunks = 0;
const listeners = new Set<() => void>();

function notify(): void {
	for (const listener of listeners) listener();
}

function subscribe(listener: () => void): () => void {
	listeners.add(listener);
	return () => {
		listeners.delete(listener);
	};
}

const chunkPending = (): boolean => pendingChunks > 0;
const serverSnapshot = (): boolean => false;

/** Render inside a lazy route's Suspense fallback to light the route bar. */
export function RouteChunkPending(): null {
	useEffect(() => {
		pendingChunks += 1;
		notify();
		return () => {
			pendingChunks -= 1;
			notify();
		};
	}, []);
	return null;
}

/**
 * Thin bar along the top of the window while the router navigates
 * (`useNavigation().state` is not idle) or a lazy page chunk loads. Shown
 * after 150 ms and for at least 300 ms (`useDelayedFlag`); renders nothing
 * otherwise. Decorative: the destination page's skeleton announces itself.
 * Needs a data router (`createBrowserRouter`, `createMemoryRouter`).
 */
export function RouteProgress() {
	const navigating = useNavigation().state !== "idle";
	const loadingChunk = useSyncExternalStore(
		subscribe,
		chunkPending,
		serverSnapshot,
	);
	const shown = useDelayedFlag(navigating || loadingChunk);
	return shown ? (
		<ProgressBar
			data-id="route-progress"
			className="fixed inset-x-0 top-0 z-100"
		/>
	) : null;
}
