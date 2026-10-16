import { IconChevronRight, IconFileText } from "@tabler/icons-react";
import { Alert } from "../../components/ui/alert";
import { Badge } from "../../components/ui/badge";
import { Card } from "../../components/ui/card";
import { EmptyState } from "../../components/ui/empty-state";
import { PanelProgress } from "../../components/ui/panel-progress";
import { Skeleton, SkeletonRegion } from "../../components/ui/skeleton";
import {
	summaryStatus,
	transcriptTitle,
} from "../../features/transcript-docs/reader-model";
import { useTranscriptDocsQuery } from "../../features/transcript-docs/transcript-docs-queries";
import { formatDateTime, formatDuration } from "../../lib/format";

export interface TranscriptListProps {
	active: boolean;
	onOpen: (key: string) => void;
}

/** Every transcript of the user, newest first; a tap opens the reader. */
export function TranscriptList({ active, onOpen }: TranscriptListProps) {
	const query = useTranscriptDocsQuery(active);
	const items = query.data ?? [];

	return (
		<Card data-id="transcripts" className="grid gap-4 p-4 sm:p-6">
			<h2 className="m-0 font-bold font-mono text-lg">Transcript</h2>
			{query.isPending ? (
				<SkeletonRegion label="Đang tải transcript…" className="grid gap-3">
					<Skeleton className="h-16" />
					<Skeleton className="h-16" />
				</SkeletonRegion>
			) : query.isError ? (
				<Alert tone="danger" data-id="transcripts-error">
					{query.error.message}
				</Alert>
			) : items.length === 0 ? (
				<EmptyState
					data-id="transcripts-empty"
					icon={<IconFileText size={28} aria-hidden="true" />}
					title="Chưa có transcript nào"
				>
					Bấm “Chép lời” trên một bản ghi ở tab Ghi âm; transcript sẽ hiện ở
					đây.
				</EmptyState>
			) : (
				<PanelProgress active={query.isFetching} aria-busy={query.isFetching}>
					<ul className="m-0 grid list-none gap-2 p-0">
						{items.map((item) => {
							const status = summaryStatus(item);
							const title = transcriptTitle(item);
							return (
								<li key={item.key}>
									<button
										type="button"
										data-id={`transcripts-item-${item.key}`}
										onClick={() => onOpen(item.key)}
										className="flex min-h-11 w-full cursor-pointer items-center gap-3 rounded-md border border-border-subtle bg-surface px-3 py-3 text-left transition-colors hover:bg-surface-subtle focus-visible:shadow-focus-primary focus-visible:outline-none"
									>
										<div className="grid min-w-0 flex-1 gap-1">
											<div
												className="line-clamp-2 break-words font-semibold"
												title={title}
											>
												{title}
											</div>
											<div className="flex flex-wrap items-center gap-x-2 gap-y-1 text-muted text-sm">
												<span>
													{formatDateTime(item.createdAt ?? undefined)}
												</span>
												<span>· {formatDuration(item.durationS)}</span>
												{item.segments > 1 ? (
													<span>· {item.segments} đoạn</span>
												) : null}
												<Badge tone={status.tone}>{status.label}</Badge>
											</div>
											{item.preview ? (
												<p className="m-0 line-clamp-2 text-muted text-sm">
													{item.preview}
												</p>
											) : null}
										</div>
										<IconChevronRight
											size={18}
											aria-hidden="true"
											className="flex-none text-icon-muted"
										/>
									</button>
								</li>
							);
						})}
					</ul>
				</PanelProgress>
			)}
		</Card>
	);
}
