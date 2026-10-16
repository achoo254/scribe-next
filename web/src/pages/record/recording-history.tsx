import { IconDownload, IconMicrophoneOff } from "@tabler/icons-react";
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
	useDeleteFiles,
	useRecordingsQuery,
} from "../../features/files/file-queries";
import type { RecordingItem } from "../../features/files/files-api";
import {
	groupByDay,
	groupSessions,
} from "../../features/files/recording-groups";
import { useT } from "../../i18n/i18n-context";
import { renderRich } from "../../i18n/rich-text";
import { downloadPaths, useDownload } from "../../lib/download";
import { useDelayedFlag } from "../../lib/use-delayed-flag";
import { RecordingRow } from "./recording-row";
import { SessionRow } from "./session-row";

/** "Lịch sử ghi": recordings by day, playback, downloads, delete. */
export function RecordingHistory() {
	const t = useT();
	const toast = useToast();
	const query = useRecordingsQuery();
	const skeleton = useDelayedFlag(query.isPending);
	const download = useDownload();
	const remove = useDeleteFiles();
	const { user } = useAuth();
	const transcribe = Boolean(user?.canTranscribe);
	const [playingId, setPlayingId] = useState<string | null>(null);
	const [seek, setSeek] = useState<{
		fileId: string;
		media: MediaSeek;
	} | null>(null);
	const [pending, setPending] = useState<{
		item: RecordingItem;
		label: string;
	} | null>(null);
	const items = query.data?.items ?? [];
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

	async function confirmDelete(): Promise<void> {
		if (!pending) return;
		await remove.mutateAsync([pending.item.id]);
		if (playingId === pending.item.id) setPlayingId(null);
		toast.show({ title: t("record.history.deleted", { name: pending.label }) });
	}

	return (
		<Card data-id="history" className="grid gap-4 p-4 sm:p-6">
			<div className="flex flex-wrap items-center justify-between gap-3">
				<h2 className="m-0 font-bold font-mono text-lg">
					{t("record.history.title")}
				</h2>
				{items.length > 0 ? (
					<Button
						size="sm"
						data-id="history-download-all"
						leadingIcon={<IconDownload size={16} aria-hidden="true" />}
						onClick={() => download(downloadPaths.allRecordings())}
					>
						{t("record.history.downloadAll")}
					</Button>
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
					<div className="grid gap-5">
						{retentionDays > 0 ? (
							<Alert tone="info" data-id="history-retention">
								{renderRich(
									t("record.history.retention", { count: retentionDays }),
								)}
							</Alert>
						) : null}
						{groups.map((group) => (
							<section
								key={group.key}
								data-id={`history-day-${group.key}`}
								className="grid gap-2"
							>
								<div className="flex flex-wrap items-center justify-between gap-2 border-border-subtle border-b pb-2">
									<h3 className="m-0 font-mono font-semibold text-muted text-[0.8125rem] first-letter:uppercase">
										{group.label}
									</h3>
									<Button
										size="sm"
										variant="ghost"
										data-id={`history-day-${group.key}-download`}
										leadingIcon={<IconDownload size={16} aria-hidden="true" />}
										onClick={() =>
											download(
												downloadPaths.recordingsBetween(group.from, group.to),
											)
										}
									>
										{t("record.history.downloadDay", {
											count: group.items.length,
										})}
									</Button>
								</div>
								<ul className="m-0 grid list-none gap-3 p-0">
									{groupSessions(group.items).map((row) =>
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
												onDelete={(item, label) => setPending({ item, label })}
											/>
										) : (
											<RecordingRow
												key={row.item.id}
												item={row.item}
												retentionDays={retentionDays}
												playing={playingId === row.item.id}
												seek={seek?.fileId === row.item.id ? seek.media : null}
												transcribe={transcribe}
												onSeek={seekTo}
												onTogglePlay={() => togglePlay(row.item.id)}
												onDelete={() =>
													setPending({ item: row.item, label: row.item.name })
												}
											/>
										),
									)}
								</ul>
							</section>
						))}
					</div>
				</PanelProgress>
			)}

			<ConfirmModal
				open={pending !== null}
				onOpenChange={(open) => {
					if (!open) setPending(null);
				}}
				title={t("record.history.deleteTitle")}
				question={t("record.history.deleteQuestion", {
					name: pending?.label ?? "",
				})}
				confirmLabel={t("common.actions.delete")}
				onConfirm={confirmDelete}
				data-id="history-delete"
			>
				{t("record.history.deleteBody")}
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
