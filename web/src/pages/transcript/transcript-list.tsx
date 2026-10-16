import {
	IconChevronRight,
	IconFileText,
	IconMicrophone,
} from "@tabler/icons-react";
import { Link } from "react-router";
import { Badge } from "../../components/ui/badge";
import { buttonClassName } from "../../components/ui/button";
import { cardClassName } from "../../components/ui/card";
import { EmptyState } from "../../components/ui/empty-state";
import { LoadError } from "../../components/ui/load-error";
import { PanelProgress } from "../../components/ui/panel-progress";
import { Skeleton, SkeletonRegion } from "../../components/ui/skeleton";
import {
	summaryStatus,
	transcriptTitle,
} from "../../features/transcript-docs/reader-model";
import { useTranscriptDocsQuery } from "../../features/transcript-docs/transcript-docs-queries";
import { useT } from "../../i18n/i18n-context";
import { PATHS } from "../../layout/app-paths";
import { PageHeader } from "../../layout/page-header";
import { cn } from "../../lib/cn";
import { formatDateTime, formatDuration } from "../../lib/format";
import { useDelayedFlag } from "../../lib/use-delayed-flag";
import { TranscriptionQuota } from "./transcription-quota";

/**
 * Hover and focus share one highlighted look, as localman-next's project
 * rows (literal classes so Tailwind sees them).
 */
const ROW_ACTIVE =
	"transition-[border-color,box-shadow] " +
	"hover:border-[color-mix(in_srgb,var(--c-primary)_45%,var(--c-border))] hover:shadow-md " +
	"has-[:focus-visible]:border-[color-mix(in_srgb,var(--c-primary)_45%,var(--c-border))]";

const SKELETON_ROWS = [0, 1, 2];

export interface TranscriptListProps {
	onOpen: (key: string) => void;
}

/** Every transcript of the user, newest first; a tap opens the reader. */
export function TranscriptList({ onOpen }: TranscriptListProps) {
	const t = useT();
	const query = useTranscriptDocsQuery();
	const skeleton = useDelayedFlag(query.isPending);
	const items = query.data ?? [];

	return (
		<section data-id="transcripts" className="grid gap-5">
			<PageHeader
				title={t("transcript.list.title")}
				description={t("transcript.list.description")}
			/>
			<TranscriptionQuota />
			{skeleton ? (
				<SkeletonRegion
					data-id="transcripts-loading"
					label={t("transcript.list.loading")}
					className="grid gap-3"
				>
					{SKELETON_ROWS.map((row) => (
						<div key={row} className={cn(cardClassName, "grid gap-2 p-4")}>
							<Skeleton className="h-4 w-64 max-w-full" />
							<Skeleton className="h-3 w-48 max-w-full" />
							<Skeleton variant="text" lines={1} />
						</div>
					))}
				</SkeletonRegion>
			) : query.isPending ? null : query.isError ? (
				<LoadError
					data-id="transcripts-error"
					title={t("transcript.list.loadFailed")}
					error={query.error}
					retrying={query.isFetching}
					onRetry={() => void query.refetch()}
				/>
			) : items.length === 0 ? (
				<EmptyState
					data-id="transcripts-empty"
					icon={<IconFileText size={28} aria-hidden="true" />}
					title={t("transcript.list.emptyTitle")}
					action={
						<Link
							to={PATHS.record}
							data-id="transcripts-empty-record"
							className={cn(
								buttonClassName({ variant: "primary" }),
								"no-underline",
							)}
						>
							<IconMicrophone size={18} aria-hidden="true" />
							{t("transcript.list.emptyAction")}
						</Link>
					}
				>
					{t("transcript.list.emptyBody")}
				</EmptyState>
			) : (
				<PanelProgress active={query.isFetching} aria-busy={query.isFetching}>
					<ul className="m-0 grid list-none gap-3 p-0">
						{items.map((item) => {
							const status = summaryStatus(item);
							const title = transcriptTitle(item);
							return (
								<li key={item.key} className={cn(cardClassName, ROW_ACTIVE)}>
									<button
										type="button"
										data-id={`transcripts-item-${item.key}`}
										onClick={() => onOpen(item.key)}
										className="flex min-h-11 w-full cursor-pointer items-center gap-3 rounded-xl bg-transparent px-4 py-3 text-left text-fg focus-visible:shadow-focus-primary focus-visible:outline-none"
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
													<span>
														·{" "}
														{t("transcript.list.segments", {
															count: item.segments,
														})}
													</span>
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
		</section>
	);
}
