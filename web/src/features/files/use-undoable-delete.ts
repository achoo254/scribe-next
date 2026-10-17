import { useQueryClient } from "@tanstack/react-query";
import { useCallback, useEffect, useRef, useState } from "react";
import { useToast } from "../../components/ui/toast";
import { useT } from "../../i18n/i18n-context";
import { ApiError } from "../../lib/api";
import { fileKeys, refreshStorage } from "./file-queries";
import { deleteFile } from "./files-api";

/** How long "Hoàn tác" stays offered before the files are really deleted. */
export const UNDO_MS = 5000;

interface PendingDelete {
	ids: readonly string[];
	toastId: number;
	/** Ends the undo window (closes the toast, which deletes). */
	timer: number;
	/** Undone, or the DELETE already went out: nothing more to do. */
	settled: boolean;
}

export interface UndoableDelete {
	/** Ids removed from the list while their undo toast is open (or being deleted). */
	hidden: ReadonlySet<string>;
	/** Hides `ids` now; deletes them when the toast closes unless undone. */
	remove: (ids: readonly string[], label: string) => void;
}

/**
 * Ids whose undo window is open or whose DELETE may not have gone out,
 * kept across app restarts. The iOS app runs no script in the background
 * (no `pagehide`; `pause`/`visibilitychange` only arrive on return), so an
 * app swiped away during the undo window would otherwise never delete.
 */
export const PENDING_DELETES_KEY = "scribe.pendingDeletes";

function readPending(): string[] {
	try {
		const stored: unknown = JSON.parse(
			window.localStorage.getItem(PENDING_DELETES_KEY) ?? "[]",
		);
		return Array.isArray(stored)
			? stored.filter((id): id is string => typeof id === "string")
			: [];
	} catch {
		return [];
	}
}

function writePending(ids: readonly string[]): void {
	try {
		if (ids.length > 0) {
			window.localStorage.setItem(PENDING_DELETES_KEY, JSON.stringify(ids));
		} else {
			window.localStorage.removeItem(PENDING_DELETES_KEY);
		}
	} catch {
		// Storage unavailable (private mode, full): the in-page flow still works.
	}
}

function rememberPending(ids: readonly string[]): void {
	writePending([...new Set([...readPending(), ...ids])]);
}

function forgetPending(ids: readonly string[]): void {
	const done = new Set(ids);
	writePending(readPending().filter((id) => !done.has(id)));
}

/** A file that is already gone counts as deleted (e.g. "Xoá tất cả" ran first). */
async function deleteQuietly(id: string, keepalive: boolean): Promise<void> {
	try {
		await deleteFile(id, fetch, { keepalive });
	} catch (error) {
		if (error instanceof ApiError && error.status === 404) return;
		throw error;
	}
}

/**
 * Delete with an undo window: the items leave the list at once, a toast
 * offers "Hoàn tác" for `UNDO_MS`, and the DELETE requests are sent only
 * when that toast closes. Undo puts the items back and sends nothing.
 * Leaving the page (unmount, tab close) sends what is still pending right
 * away, with `keepalive` so the browser finishes it. Ids still pending when
 * the page or app was closed before that (`PENDING_DELETES_KEY`) are
 * deleted, without a new undo, the next time the history mounts.
 */
export function useUndoableDelete(): UndoableDelete {
	const t = useT();
	const toast = useToast();
	const queryClient = useQueryClient();
	const [hidden, setHidden] = useState<ReadonlySet<string>>(() => new Set());
	const pending = useRef(new Set<PendingDelete>());
	// Latest values for callbacks created once (toast and unload handlers).
	const latest = useRef({ t, toast, queryClient });
	latest.current = { t, toast, queryClient };

	const show = useCallback((ids: readonly string[], visible: boolean) => {
		setHidden((current) => {
			const next = new Set(current);
			for (const id of ids) {
				if (visible) next.delete(id);
				else next.add(id);
			}
			return next;
		});
	}, []);

	const commit = useCallback(
		async (entry: PendingDelete) => {
			if (entry.settled) return;
			entry.settled = true;
			pending.current.delete(entry);
			// All at once with `keepalive`: closing the tab right after the undo
			// window still deletes every part of a session, not only the first.
			const results = await Promise.allSettled(
				entry.ids.map((id) => deleteQuietly(id, true)),
			);
			forgetPending(entry.ids);
			const failed = results.filter((r) => r.status === "rejected");
			const { t: tr, toast: toasts, queryClient: client } = latest.current;
			if (failed.length > 0) {
				toasts.show({ tone: "error", title: tr("errors.files.delete") });
			}
			// Keep the deleted items hidden until the refreshed list no longer has them.
			await refreshStorage(client, ["recordings", "managed"]).catch(
				() => undefined,
			);
			show(entry.ids, true);
		},
		[show],
	);

	const flush = useCallback(() => {
		for (const entry of pending.current) {
			window.clearTimeout(entry.timer);
			entry.settled = true;
			const { ids } = entry;
			void Promise.allSettled(ids.map((id) => deleteQuietly(id, true))).then(
				() => forgetPending(ids),
			);
			latest.current.toast.dismiss(entry.toastId);
		}
		pending.current.clear();
	}, []);

	// Deletes the page or app was closed on before they were sent.
	useEffect(() => {
		const left = readPending();
		if (left.length === 0) return;
		show(left, false);
		// The list read on mount may answer before these deletes land: drop
		// it, so the refresh after them is a new read, not that one.
		void latest.current.queryClient
			.cancelQueries({ queryKey: fileKeys.recordings })
			.then(() => commit({ ids: left, toastId: 0, timer: 0, settled: false }));
	}, [commit, show]);

	useEffect(() => {
		window.addEventListener("pagehide", flush);
		return () => {
			window.removeEventListener("pagehide", flush);
			const hadPending = pending.current.size > 0;
			flush();
			if (hadPending) {
				void refreshStorage(latest.current.queryClient, [
					"recordings",
					"managed",
				]).catch(() => undefined);
			}
		};
	}, [flush]);

	const remove = useCallback(
		(ids: readonly string[], label: string) => {
			if (ids.length === 0) return;
			show(ids, false);
			rememberPending(ids);
			const entry: PendingDelete = {
				ids,
				toastId: 0,
				timer: 0,
				settled: false,
			};
			pending.current.add(entry);
			const { t: tr, toast: toasts } = latest.current;
			// Our own timer, not the toast's: Radix pauses its timer while the
			// pointer rests on the toast or the window is in the background,
			// and the delete must not wait for that.
			entry.toastId = toasts.show({
				title: tr("record.history.deleted", { name: label }),
				duration: Number.POSITIVE_INFINITY,
				action: {
					label: tr("record.history.undo"),
					onClick: () => {
						window.clearTimeout(entry.timer);
						if (entry.settled) return;
						entry.settled = true;
						pending.current.delete(entry);
						forgetPending(ids);
						show(ids, true);
					},
				},
				onClose: () => {
					window.clearTimeout(entry.timer);
					void commit(entry);
				},
			});
			entry.timer = window.setTimeout(
				() => latest.current.toast.dismiss(entry.toastId),
				UNDO_MS,
			);
		},
		[commit, show],
	);

	return { hidden, remove };
}
