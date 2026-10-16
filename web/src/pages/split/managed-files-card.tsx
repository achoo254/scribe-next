import { IconDownload, IconTrash } from "@tabler/icons-react";
import { useMemo, useState } from "react";
import { Button } from "../../components/ui/button";
import { Card } from "../../components/ui/card";
import { ConfirmModal } from "../../components/ui/confirm-modal";
import {
	DropdownMenuItem,
	DropdownMenuSeparator,
} from "../../components/ui/dropdown-menu";
import { LoadError } from "../../components/ui/load-error";
import { PanelProgress } from "../../components/ui/panel-progress";
import { RowActionsMenu } from "../../components/ui/row-actions-menu";
import { useToast } from "../../components/ui/toast";
import {
	useDeleteFiles,
	useManagedFilesQuery,
} from "../../features/files/file-queries";
import { groupOutputsByJob } from "../../features/files/recording-groups";
import { useT } from "../../i18n/i18n-context";
import { downloadPaths, useDownload } from "../../lib/download";
import { formatDateTime, formatSize } from "../../lib/format";

interface PendingDelete {
	ids: string[];
	question: string;
	/** Toast once the files are gone. */
	done: string;
}

/** The details column has inline-size containment so a long file name
    truncates instead of widening the row past the screen. */
const ROW =
	"flex items-center gap-2 border-border-subtle border-t py-2 first:border-t-0";

/**
 * Uploads and split segments: they count against the quota but are not
 * recordings, so they are listed (and can be deleted) here. Re-read each
 * time the page opens (`refetchOnMount: "always"`); nothing shows until
 * the first answer, and the card is left out when there is no such file.
 */
export function ManagedFilesCard() {
	const t = useT();
	const toast = useToast();
	const query = useManagedFilesQuery();
	const download = useDownload();
	const remove = useDeleteFiles();
	const [pending, setPending] = useState<PendingDelete | null>(null);
	const files = query.data ?? [];
	const uploads = useMemo(
		() => files.filter((file) => file.kind === "upload"),
		[files],
	);
	const outputs = useMemo(() => groupOutputsByJob(files), [files]);

	if (query.isError) {
		return (
			<LoadError
				data-id="managed-error"
				title={t("split.managed.loadFailed")}
				error={query.error}
				retrying={query.isFetching}
				onRetry={() => void query.refetch()}
			/>
		);
	}
	if (files.length === 0) return null;

	return (
		<Card data-id="managed-files" className="grid gap-4 p-4 sm:p-6">
			<div className="grid gap-1">
				<h2 className="m-0 font-bold font-mono text-lg">
					{t("split.managed.title")}
				</h2>
				<p className="m-0 text-muted text-sm">
					{t("split.managed.description")}
				</p>
			</div>
			<PanelProgress active={query.isFetching} aria-busy={query.isFetching}>
				<div className="grid gap-4">
					{uploads.length > 0 ? (
						<section className="grid gap-1">
							<h3 className="m-0 font-mono font-semibold text-sm">
								{t("split.managed.uploads")}
							</h3>
							<ul className="m-0 list-none p-0">
								{uploads.map((file) => {
									const id = `managed-upload-${file.id}`;
									return (
										<li key={file.id} data-id={id} className={ROW}>
											<div className="min-w-0 flex-1 contain-inline-size">
												<div
													className="truncate font-semibold"
													title={file.name}
												>
													{file.name}
												</div>
												<div className="text-muted text-sm">
													{formatSize(file.size)}
													{file.createdAt
														? ` · ${formatDateTime(file.createdAt)}`
														: ""}
												</div>
											</div>
											<RowActionsMenu
												label={t("split.managed.menu", { name: file.name })}
												data-id={`${id}-menu`}
											>
												<DropdownMenuItem
													data-id={`${id}-download`}
													icon={<IconDownload size={16} aria-hidden="true" />}
													onSelect={() => download(downloadPaths.file(file.id))}
												>
													{t("split.managed.download")}
												</DropdownMenuItem>
												<DropdownMenuSeparator />
												<DropdownMenuItem
													danger
													data-id={`${id}-delete`}
													icon={<IconTrash size={16} aria-hidden="true" />}
													onSelect={() =>
														setPending({
															ids: [file.id],
															question: t("split.managed.deleteFile", {
																name: file.name,
															}),
															done: t("split.managed.fileDeleted", {
																name: file.name,
															}),
														})
													}
												>
													{t("common.actions.delete")}
												</DropdownMenuItem>
											</RowActionsMenu>
										</li>
									);
								})}
							</ul>
						</section>
					) : null}
					{outputs.length > 0 ? (
						<section className="grid gap-1">
							<h3 className="m-0 font-mono font-semibold text-sm">
								{t("split.managed.outputs")}
							</h3>
							<ul className="m-0 list-none p-0">
								{outputs.map((group) => (
									<li
										key={group.jobId}
										data-id={`managed-job-${group.jobId}`}
										className={ROW}
									>
										<div className="min-w-0 flex-1 contain-inline-size">
											<div className="font-semibold">
												{t("split.managed.segments", { count: group.count })}
											</div>
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
													question: t("split.managed.deleteSegments", {
														count: group.count,
													}),
													done: t("split.managed.segmentsDeleted", {
														count: group.count,
													}),
												})
											}
										>
											{t("split.managed.deleteAll")}
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
				title={t("split.managed.deleteTitle")}
				question={pending?.question ?? ""}
				confirmLabel={t("common.actions.delete")}
				onConfirm={async () => {
					if (!pending) return;
					await remove.mutateAsync(pending.ids);
					toast.show({ title: pending.done });
				}}
				data-id="managed-delete"
			>
				{t("split.managed.deleteBody")}
			</ConfirmModal>
		</Card>
	);
}
