import { useEffect, useMemo, useState } from "react";
import { Alert } from "../../components/ui/alert";
import { Button } from "../../components/ui/button";
import { Card } from "../../components/ui/card";
import { ConfirmModal } from "../../components/ui/confirm-modal";
import { PanelProgress } from "../../components/ui/panel-progress";
import {
	useDeleteFiles,
	useManagedFilesQuery,
} from "../../features/files/file-queries";
import { groupOutputsByJob } from "../../features/files/recording-groups";
import { download, downloadPaths } from "../../lib/download";
import { formatDateTime, formatSize } from "../../lib/format";

interface PendingDelete {
	ids: string[];
	question: string;
}

const ROW =
	"flex flex-wrap items-center gap-x-3 gap-y-1 border-border-subtle border-t py-2 first:border-t-0";

/**
 * Uploads and split segments: they count against the quota but are not
 * recordings, so they are listed (and can be deleted) here. Re-read each
 * time the tab opens.
 */
export function ManagedFilesCard({ active }: { active: boolean }) {
	const query = useManagedFilesQuery();
	const remove = useDeleteFiles();
	const [pending, setPending] = useState<PendingDelete | null>(null);
	const files = query.data ?? [];
	const uploads = useMemo(
		() => files.filter((file) => file.kind === "upload"),
		[files],
	);
	const outputs = useMemo(() => groupOutputsByJob(files), [files]);
	const { refetch } = query;

	useEffect(() => {
		if (active) void refetch();
	}, [active, refetch]);

	if (query.isError) {
		return <Alert tone="danger">{query.error.message}</Alert>;
	}
	if (files.length === 0) return null;

	return (
		<Card data-id="managed-files" className="grid gap-4 p-4 sm:p-6">
			<h2 className="m-0 font-bold font-mono text-lg">Tệp trên hệ thống</h2>
			<p className="m-0 text-muted text-sm">
				Tệp bạn tải lên để cắt và các đoạn đã cắt vẫn tính vào dung lượng. Xoá
				bớt để giải phóng (tự động xoá sau 14 ngày nếu không xoá tay).
			</p>
			<PanelProgress active={query.isFetching} aria-busy={query.isFetching}>
				<div className="grid gap-4">
					{uploads.length > 0 ? (
						<section className="grid gap-1">
							<h3 className="m-0 font-mono font-semibold text-sm">
								Tệp đã tải lên
							</h3>
							<ul className="m-0 list-none p-0">
								{uploads.map((file) => (
									<li
										key={file.id}
										data-id={`managed-upload-${file.id}`}
										className={ROW}
									>
										<div className="min-w-0 flex-1 basis-48">
											<div className="truncate font-semibold" title={file.name}>
												{file.name}
											</div>
											<div className="text-muted text-sm">
												{formatSize(file.size)}
												{file.createdAt
													? ` · ${formatDateTime(file.createdAt)}`
													: ""}
											</div>
										</div>
										<Button
											size="sm"
											variant="ghost"
											data-id={`managed-upload-${file.id}-download`}
											onClick={() => download(downloadPaths.file(file.id))}
										>
											Tải
										</Button>
										<Button
											size="sm"
											variant="ghost"
											className="text-error!"
											data-id={`managed-upload-${file.id}-delete`}
											onClick={() =>
												setPending({
													ids: [file.id],
													question: `Xoá “${file.name}”?`,
												})
											}
										>
											Xoá
										</Button>
									</li>
								))}
							</ul>
						</section>
					) : null}
					{outputs.length > 0 ? (
						<section className="grid gap-1">
							<h3 className="m-0 font-mono font-semibold text-sm">
								Đoạn đã cắt
							</h3>
							<ul className="m-0 list-none p-0">
								{outputs.map((group) => (
									<li
										key={group.jobId}
										data-id={`managed-job-${group.jobId}`}
										className={ROW}
									>
										<div className="min-w-0 flex-1 basis-48">
											<div className="font-semibold">{group.count} đoạn</div>
											<div className="text-muted text-sm">
												{formatSize(group.bytes)}
												{group.createdAt
													? ` · ${formatDateTime(group.createdAt)}`
													: ""}
											</div>
										</div>
										<Button
											size="sm"
											variant="ghost"
											className="text-error!"
											data-id={`managed-job-${group.jobId}-delete`}
											onClick={() =>
												setPending({
													ids: group.ids,
													question: `Xoá ${group.count} đoạn đã cắt?`,
												})
											}
										>
											Xoá tất cả
										</Button>
									</li>
								))}
							</ul>
						</section>
					) : null}
				</div>
			</PanelProgress>
			<ConfirmModal
				open={pending !== null}
				onOpenChange={(open) => {
					if (!open) setPending(null);
				}}
				title="Xoá tệp"
				question={pending?.question ?? ""}
				confirmLabel="Xoá"
				onConfirm={async () => {
					if (pending) await remove.mutateAsync(pending.ids);
				}}
				data-id="managed-delete"
			>
				Dung lượng được giải phóng ngay. Hành động này không thể hoàn tác.
			</ConfirmModal>
		</Card>
	);
}
