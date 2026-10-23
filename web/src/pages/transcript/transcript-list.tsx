import { IconFileText, IconMicrophone } from "@tabler/icons-react";
import { useMemo } from "react";
import { Link } from "react-router";
import { buttonClassName } from "../../components/ui/button";
import { EmptyState } from "../../components/ui/empty-state";
import { LoadError } from "../../components/ui/load-error";
import { PanelProgress } from "../../components/ui/panel-progress";
import { Skeleton, SkeletonRegion } from "../../components/ui/skeleton";
import { groupByDay } from "../../features/files/recording-groups";
import { useTranscriptDocsQuery } from "../../features/transcript-docs/transcript-docs-queries";
import { useT } from "../../i18n/i18n-context";
import { PATHS } from "../../layout/app-paths";
import { PageHeader } from "../../layout/page-header";
import { cn } from "../../lib/cn";
import { formatDuration } from "../../lib/format";
import { useDelayedFlag } from "../../lib/use-delayed-flag";
import { SearchEntry } from "../search/search-entry";
import { TranscriptListRow } from "./transcript-list-row";

const SKELETON_ROWS = [0, 1, 2];

export interface TranscriptListProps {
	onOpen: (key: string) => void;
}

/**
 * Every transcript of the user, newest first, in day blocks like "Lịch sử
 * ghi"; a tap opens the reader.
 */
export function TranscriptList({ onOpen }: TranscriptListProps) {
	const t = useT();
	const query = useTranscriptDocsQuery();
	const skeleton = useDelayedFlag(query.isPending);
	const items = query.data;
	const groups = useMemo(() => groupByDay(items ?? []), [items]);

	return (
		<section data-id="transcripts" className="grid gap-5">
			<PageHeader
				title={t("transcript.list.title")}
				description={t("transcript.list.description")}
			/>
			<SearchEntry />
			{skeleton ? (
				<SkeletonRegion
					data-id="transcripts-loading"
					label={t("transcript.list.loading")}
					className="grid"
				>
					{/* The shape of one day block: header, then rows. */}
					<div className="rounded-lg border border-border-subtle">
						<div className="grid gap-1 border-border-subtle border-b px-3 py-2 sm:px-4">
							<Skeleton className="h-4 w-40" />
							<Skeleton className="h-3 w-32" />
						</div>
						{SKELETON_ROWS.map((row) => (
							<div
								key={row}
								className="grid gap-2 border-border-subtle border-t px-3 py-3 first:border-t-0 sm:px-4"
							>
								<Skeleton className="h-4 w-64 max-w-full" />
								<Skeleton className="h-3 w-48 max-w-full" />
								<Skeleton variant="text" lines={1} />
							</div>
						))}
					</div>
				</SkeletonRegion>
			) : query.isPending ? null : query.isError ? (
				<LoadError
					data-id="transcripts-error"
					title={t("transcript.list.loadFailed")}
					error={query.error}
					retrying={query.isFetching}
					onRetry={() => void query.refetch()}
				/>
			) : groups.length === 0 ? (
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
					<div className="grid gap-4">
						{groups.map((group) => {
							const dayId = `transcripts-day-${group.key}`;
							return (
								<section
									key={group.key}
									data-id={dayId}
									aria-labelledby={`${dayId}-heading`}
									className="rounded-lg border border-border-subtle bg-surface"
								>
									{/* Stays under the top bar while the day's rows scroll by,
									    then the next day's header pushes it away. */}
									<div
										data-id={`${dayId}-header`}
										className="sticky top-[calc(var(--topbar-h)+env(safe-area-inset-top))] z-10 grid rounded-t-lg border-border-subtle border-b bg-surface-subtle px-3 py-2 sm:px-4"
									>
										<h2
											id={`${dayId}-heading`}
											className="m-0 flex flex-wrap items-baseline gap-x-2 font-semibold text-base text-fg"
										>
											<span>{group.label}</span>
											<span className="font-mono font-normal text-muted text-sm">
												{group.dateLabel}
											</span>
										</h2>
										<p
											data-id={`${dayId}-meta`}
											className="m-0 text-muted text-xs"
										>
											{t("transcript.list.dayMeta", {
												count: group.items.length,
												duration: formatDuration(group.durationS),
											})}
										</p>
									</div>
									<ul className="m-0 grid list-none p-0">
										{group.items.map((item) => (
											<TranscriptListRow
												key={item.key}
												item={item}
												onOpen={onOpen}
											/>
										))}
									</ul>
								</section>
							);
						})}
					</div>
				</PanelProgress>
			)}
		</section>
	);
}
