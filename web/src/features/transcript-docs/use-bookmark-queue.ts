import { useEffect, useRef, useSyncExternalStore } from "react";
import { BookmarkQueue, type BookmarkQueueState } from "./bookmark-queue";

export interface BookmarkQueueApi extends BookmarkQueueState {
	queue: BookmarkQueue;
}

/**
 * One bookmark queue for the lifetime of the component (the signed-in
 * recorder state); it stops sending when that unmounts (sign-out).
 */
export function useBookmarkQueue(): BookmarkQueueApi {
	const ref = useRef<BookmarkQueue | null>(null);
	ref.current ??= new BookmarkQueue();
	const queue = ref.current;
	const state = useSyncExternalStore(queue.subscribe, queue.getState);
	useEffect(() => {
		queue.resume();
		return () => queue.dispose();
	}, [queue]);
	return { ...state, queue };
}
