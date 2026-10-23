import { IconChevronRight } from "@tabler/icons-react";
import { Badge } from "../../components/ui/badge";
import {
	hasDefaultSessionTitle,
	summaryStatus,
	transcriptTitle,
} from "../../features/transcript-docs/reader-model";
import type { TranscriptSummary } from "../../features/transcript-docs/transcript-docs-api";
import { useT } from "../../i18n/i18n-context";
import { formatDuration, formatTimeOfDay } from "../../lib/format";

/**
 * Rows sit edge to edge, so focus draws inside the row (an outer ring would
 * hide under the sticky day header and the next row). The scroll margin keeps
 * a focused row clear of the top bar plus the day header.
 */
const ROW_BUTTON =
	"flex min-h-11 w-full cursor-pointer items-center gap-3 bg-transparent px-3 py-3 text-left text-fg transition-colors sm:px-4 " +
	"scroll-mt-[calc(var(--topbar-h)+env(safe-area-inset-top)+4.5rem)] hover:bg-fg/6 " +
	"focus-visible:outline-2 focus-visible:outline-primary focus-visible:-outline-offset-2";

export interface TranscriptListRowProps {
	item: TranscriptSummary;
	onOpen: (key: string) => void;
}

/**
 * One transcript inside a day block. The day heading carries the date, so
 * the row shows only the time: in the default iPhone name ("Bản ghi trên
 * iPhone 09:41"), or leading the details line for a named transcript.
 */
export function TranscriptListRow({ item, onOpen }: TranscriptListRowProps) {
	const t = useT();
	const status = summaryStatus(item);
	const title = transcriptTitle(item, { withDate: false });
	const time = hasDefaultSessionTitle(item)
		? ""
		: formatTimeOfDay(item.createdAt);
	const details = [
		formatDuration(item.durationS),
		item.segments > 1
			? t("transcript.list.segments", { count: item.segments })
			: "",
	].filter(Boolean);

	return (
		<li className="border-border-subtle border-t first:border-t-0 last:*:rounded-b-lg">
			<button
				type="button"
				data-id={`transcripts-item-${item.key}`}
				onClick={() => onOpen(item.key)}
				className={ROW_BUTTON}
			>
				<div className="grid min-w-0 flex-1 gap-1">
					<div className="flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1">
						<span
							className="line-clamp-2 min-w-0 break-words font-semibold"
							title={title}
						>
							{title}
						</span>
						<Badge tone={status.tone}>{status.label}</Badge>
					</div>
					<div className="text-muted text-sm">
						{time ? (
							<>
								<span className="font-mono text-fg">{time}</span>
								{" · "}
							</>
						) : null}
						{details.join(" · ")}
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
}
