import {
	IconDownload,
	IconMicrophoneOff,
	IconTrash,
} from "@tabler/icons-react";
import { useMemo, useState } from "react";
import { useAuth } from "../../auth/auth-context";
import type { MediaSeek } from "../../components/media-player";
import { Alert } from "../../components/ui/alert";
import { Button } from "../../components/ui/button";
import { Card } from "../../components/ui/card";
import { ConfirmModal } from "../../components/ui/confirm-modal";
import { EmptyState } from "../../components/ui/empty-state";
import { LoadError } from "../../components/ui/load-error";
import { PanelProgress } from "../../components/ui/panel-progress";
import { Skeleton, SkeletonRegion } from "../../components/ui/skeleton";
import { useToast } from "../../components/ui/toast";
import {
	useDeleteAllFiles,
	useRecordingsQuery,
} from "../../features/files/file-queries";
import {
	groupByDay,
	groupSessions,
} from "../../features/files/recording-groups";
import { useUndoableDelete } from "../../features/files/use-undoable-delete";
import { useT } from "../../i18n/i18n-context";
import { renderRich } from "../../i18n/rich-text";
import { downloadPaths, useDownload } from "../../lib/download";
import { formatDuration, formatSize } from "../../lib/format";
import { useDelayedFlag } from "../../lib/use-delayed-flag";
import { RecordingRow } from "./recording-row";
import { SessionRow } from "./session-row";

/**
 * "Lịch sử ghi": recordings and uploaded files by day, playback, downloads,
 * delete with undo, and "Xoá tất cả".
 */
export function RecordingHistory() {
	const t = useT();
	const toast = useToast();
	const query = useRecordingsQuery();
	const skeleton = useDelayedFlag(query.isPending);
	const download = useDownload();
	const { hidden, remove } = useUndoableDelete();
	const deleteAll = useDeleteAllFiles();
	const [confirmAll, setConfirmAll] = useState(false);
	const { user } = useAuth();
	const transcribe = Boolean(user?.canTranscribe);
	const [selectedId, setPlayingId] = useState<string | null>(null);
	const [seek, setSeek] = useState<{
		fileId: string;
		media: MediaSeek;
	} | null>(null);
	const allItems = query.data?.items ?? [];
	const items = useMemo(
		() => allItems.filter((item) => !hidden.has(item.id)),
		[allItems, hidden],
	);
	// A deleted item stops playing; undo shows it closed.
	const playingId = selectedId && !hidden.has(selectedId) ? selectedId : null;
	const totalBytes = items.reduce((sum, item) => sum + item.size, 0);
	const retentionDays = query.data?.retentionDays ?? 0;
	const groups = useMemo(() => groupByDay(items), [items]);
	const togglePlay = (id: string) => {
		setSeek(null);
		setPlayingId((current) => (current === id ? null : id));
	};
	const seekTo = (fileId: string, time: number) => {
		setPlayingId(fileId);
		setSeek({ fileId, media: { time } });
	};

	async function confirmDeleteAll(): Promise<void> {
		const result = await deleteAll.mutateAsync();
		setPlayingId(null);
		toast.show({
			title: t("record.history.deletedAll", { count: result.deleted }),
			description:
				result.skipped > 0
					? t("record.history.deletedAllSkipped", { count: result.skipped })
					: undefined,
		});
	}

	return (
		<Card data-id="history" className="grid gap-4 p-4 sm:p-6">
			<div className="flex flex-wrap items-center justify-between gap-3">
				<h2 className="m-0 font-bold font-mono text-lg">
					{t("record.history.title")}
				</h2>
				{items.length > 0 ? (
					<div className="flex flex-wrap items-center gap-2">
						<Button
							size="sm"
							data-id="history-download-all"
							leadingIcon={<IconDownload size={16} aria-hidden="true" />}
							onClick={() => download(downloadPaths.allRecordings())}
						>
							{t("record.history.downloadAll")}
						</Button>
						<Button
							size="sm"
							variant="ghost"
							className="text-error!"
							data-id="history-delete-all"
							leadingIcon={<IconTrash size={16} aria-hidden="true" />}
							onClick={() => setConfirmAll(true)}
						>
							{t("record.history.deleteAll")}
						</Button>
					</div>
				) : null}
			</div>

			{skeleton ? (
				<HistorySkeleton />
			) : query.isPending ? null : query.isError ? (
				<LoadError
					data-id="history-error"
					title={t("record.history.loadFailed")}
					error={query.error}
					retrying={query.isFetching}
					onRetry={() => void query.refetch()}
				/>
			) : items.length === 0 ? (
				<EmptyState
					data-id="history-empty"
					icon={<IconMicrophoneOff size={28} aria-hidden="true" />}
					title={t("record.history.emptyTitle")}
				>
					{t("record.history.emptyBody")}
				</EmptyState>
			) : (
				<PanelProgress active={query.isFetching} aria-busy={query.isFetching}>
					<div className="grid gap-4">
						{retentionDays > 0 ? (
							<Alert tone="info" data-id="history-retention">
								{renderRich(
									t("record.history.retention", { count: retentionDays }),
								)}
							</Alert>
						) : null}
						{groups.map((group) => {
							const rows = groupSessions(group.items);
							const dayId = `history-day-${group.key}`;
							return (
								<section
									key={group.key}
									data-id={dayId}
									aria-labelledby={`${dayId}-heading`}
									className="rounded-lg border border-border-subtle"
								>
									{/* Stays under the top bar while the day's rows scroll by,
									    then the next day's header pushes it away. */}
									<div
										data-id={`${dayId}-header`}
										className="sticky top-[calc(var(--topbar-h)+env(safe-area-inset-top))] z-10 flex flex-wrap items-center justify-between gap-x-3 gap-y-1 rounded-t-lg border-border-subtle border-b bg-surface-subtle px-3 py-2 sm:px-4"
									>
										<div className="grid min-w-0">
											<h3
												id={`${dayId}-heading`}
												className="m-0 flex flex-wrap items-baseline gap-x-2 font-semibold text-base text-fg"
											>
												<span>{group.label}</span>
												<span className="font-mono font-normal text-muted text-sm">
													{group.dateLabel}
												</span>
											</h3>
											<p
												data-id={`${dayId}-meta`}
												className="m-0 text-muted text-xs"
											>
												{t("record.history.dayMeta", {
													count: rows.length,
													duration: formatDuration(group.durationS),
												})}
											</p>
										</div>
										<Button
											size="sm"
											variant="ghost"
											data-id={`${dayId}-download`}
											aria-label={t("record.history.downloadDayLabel", {
												date: group.dateLabel,
											})}
											leadingIcon={
												<IconDownload size={16} aria-hidden="true" />
											}
											onClick={() =>
												download(
													downloadPaths.recordingsBetween(group.from, group.to),
												)
											}
										>
											{t("record.history.downloadDay")}
										</Button>
									</div>
									<ul className="m-0 grid list-none gap-3 p-3 sm:p-4">
										{rows.map((row) =>
											row.kind === "session" ? (
												<SessionRow
													key={row.sessionId}
													row={row}
													retentionDays={retentionDays}
													playingId={playingId}
													seek={seek}
													transcribe={transcribe}
													onSeek={seekTo}
													onTogglePlay={togglePlay}
													onDelete={(ids, label) => remove(ids, label)}
												/>
											) : (
												<RecordingRow
													key={row.item.id}
													item={row.item}
													retentionDays={retentionDays}
													playing={playingId === row.item.id}
													seek={
														seek?.fileId === row.item.id ? seek.media : null
													}
													transcribe={transcribe}
													onSeek={seekTo}
													onTogglePlay={() => togglePlay(row.item.id)}
													onDelete={() =>
														remove(
															[row.item.id],
															row.item.title || row.item.name,
														)
													}
												/>
											),
										)}
									</ul>
								</section>
							);
						})}
					</div>
				</PanelProgress>
			)}

			<ConfirmModal
				open={confirmAll}
				onOpenChange={setConfirmAll}
				title={t("record.history.deleteAllTitle")}
				question={t("record.history.deleteAllQuestion", {
					count: items.length,
					size: formatSize(totalBytes),
				})}
				confirmLabel={t("record.history.deleteAll")}
				onConfirm={confirmDeleteAll}
				data-id="history-delete-all-confirm"
			>
				{t("record.history.deleteAllBody")}
			</ConfirmModal>
		</Card>
	);
}

/** First load: three rows in the shape of `RecordingRow` (name, details, actions). */
function HistorySkeleton() {
	const t = useT();
	return (
		<SkeletonRegion
			data-id="history-loading"
			label={t("record.history.loading")}
			className="grid gap-4"
		>
			{[0, 1, 2].map((row) => (
				<div key={row} className="flex items-center gap-3">
					<div className="grid min-w-0 flex-1 gap-2">
						<Skeleton className="h-4 w-56 max-w-full" />
						<Skeleton className="h-3 w-72 max-w-full" />
					</div>
					<Skeleton className="h-8 w-24 shrink-0" />
					<Skeleton className="size-9 shrink-0" />
				</div>
			))}
		</SkeletonRegion>
	);
}
