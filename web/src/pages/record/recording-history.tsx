import { IconDownload, IconMicrophoneOff } from "@tabler/icons-react";
import { useMemo, useState } from "react";
import { Alert } from "../../components/ui/alert";
import { Button } from "../../components/ui/button";
import { Card } from "../../components/ui/card";
import { ConfirmModal } from "../../components/ui/confirm-modal";
import { EmptyState } from "../../components/ui/empty-state";
import { PanelProgress } from "../../components/ui/panel-progress";
import { Skeleton, SkeletonRegion } from "../../components/ui/skeleton";
import {
	useDeleteFiles,
	useRecordingsQuery,
} from "../../features/files/file-queries";
import type { RecordingItem } from "../../features/files/files-api";
import { groupByDay } from "../../features/files/recording-groups";
import { download, downloadPaths } from "../../lib/download";
import { RecordingRow } from "./recording-row";

/** "Lịch sử ghi": recordings by day, playback, downloads, delete. */
export function RecordingHistory() {
	const query = useRecordingsQuery();
	const remove = useDeleteFiles();
	const [playingId, setPlayingId] = useState<string | null>(null);
	const [pending, setPending] = useState<RecordingItem | null>(null);
	const items = query.data?.items ?? [];
	const retentionDays = query.data?.retentionDays ?? 0;
	const groups = useMemo(() => groupByDay(items), [items]);

	async function confirmDelete(): Promise<void> {
		if (!pending) return;
		await remove.mutateAsync([pending.id]);
		if (playingId === pending.id) setPlayingId(null);
	}

	return (
		<Card data-id="history" className="grid gap-4 p-4 sm:p-6">
			<div className="flex flex-wrap items-center justify-between gap-3">
				<h2 className="m-0 font-bold font-mono text-lg">Lịch sử ghi</h2>
				{items.length > 0 ? (
					<Button
						size="sm"
						data-id="history-download-all"
						leadingIcon={<IconDownload size={16} aria-hidden="true" />}
						onClick={() => download(downloadPaths.allRecordings())}
					>
						Tải tất cả
					</Button>
				) : null}
			</div>

			{query.isPending ? (
				<SkeletonRegion label="Đang tải lịch sử ghi…" className="grid gap-3">
					<Skeleton className="h-14" />
					<Skeleton className="h-14" />
					<Skeleton className="h-14" />
				</SkeletonRegion>
			) : query.isError ? (
				<Alert tone="danger" data-id="history-error">
					{query.error.message}
				</Alert>
			) : items.length === 0 ? (
				<EmptyState
					data-id="history-empty"
					icon={<IconMicrophoneOff size={28} aria-hidden="true" />}
					title="Chưa có bản ghi nào"
				>
					Bản ghi sẽ hiện ở đây sau khi bạn bấm “Dừng ghi”.
				</EmptyState>
			) : (
				<PanelProgress active={query.isFetching} aria-busy={query.isFetching}>
					<div className="grid gap-5">
						{retentionDays > 0 ? (
							<Alert tone="info" data-id="history-retention">
								Bản ghi được{" "}
								<strong>tự động xoá sau {retentionDays} ngày</strong> kể từ khi
								tạo. Hãy tải về nếu muốn lưu giữ lâu dài.
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
										Tải ngày ({group.items.length})
									</Button>
								</div>
								<ul className="m-0 grid list-none gap-3 p-0">
									{group.items.map((item) => (
										<RecordingRow
											key={item.id}
											item={item}
											retentionDays={retentionDays}
											playing={playingId === item.id}
											onTogglePlay={() =>
												setPlayingId((current) =>
													current === item.id ? null : item.id,
												)
											}
											onDelete={() => setPending(item)}
										/>
									))}
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
				title="Xoá bản ghi"
				question={`Xoá “${pending?.name ?? ""}”?`}
				confirmLabel="Xoá"
				onConfirm={confirmDelete}
				data-id="history-delete"
			>
				Hành động này không thể hoàn tác.
			</ConfirmModal>
		</Card>
	);
}
