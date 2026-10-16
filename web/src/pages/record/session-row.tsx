import { IconChevronDown, IconChevronRight } from "@tabler/icons-react";
import { useEffect, useMemo, useState } from "react";
import type { MediaSeek } from "../../components/media-player";
import { Badge } from "../../components/ui/badge";
import { Button } from "../../components/ui/button";
import type { RecordingItem } from "../../features/files/files-api";
import {
	type SessionRow as SessionRowData,
	sessionTitle,
} from "../../features/files/recording-groups";
import { useT } from "../../i18n/i18n-context";
import { formatDuration, formatSize } from "../../lib/format";
import { RecordingRow } from "./recording-row";
import { TranscriptPanel } from "./transcript-panel";

export interface SessionRowProps {
	row: SessionRowData;
	retentionDays: number;
	playingId: string | null;
	seek: { fileId: string; media: MediaSeek } | null;
	transcribe: boolean;
	onSeek: (fileId: string, seconds: number) => void;
	onTogglePlay: (id: string) => void;
	onDelete: (item: RecordingItem, label: string) => void;
}

/**
 * One iOS recording session: its segments (one per minute by default)
 * collapsed into a single row that opens into "Phần 01…".
 */
export function SessionRow({
	row,
	retentionDays,
	playingId,
	seek,
	transcribe,
	onSeek,
	onTogglePlay,
	onDelete,
}: SessionRowProps) {
	const t = useT();
	const [open, setOpen] = useState(false);
	const parts = useMemo(
		() =>
			row.parts.map(({ item }) => ({
				fileId: item.id,
				durationS: item.durationS,
			})),
		[row.parts],
	);
	// A transcript click plays one of the segments: open the list to show it.
	// Every new seek reopens it, even after "Thu gọn".
	useEffect(() => {
		if (seek && row.parts.some(({ item }) => item.id === seek.fileId))
			setOpen(true);
	}, [seek, row.parts]);
	const title = sessionTitle(row.createdAt);
	const id = `history-session-${row.sessionId.slice(0, 8)}`;

	return (
		<li
			data-id={id}
			className="grid gap-2 border-border-subtle border-t pt-3 first:border-t-0 first:pt-0"
		>
			<div className="flex items-center gap-2">
				{/* Inline-size containment: a long title truncates instead of
				    widening the row past the screen. */}
				<div className="min-w-0 flex-1 contain-inline-size">
					<div className="flex min-w-0 items-center gap-2">
						<Badge tone="primary">{t("record.session.badge")}</Badge>
						<span className="truncate font-semibold" title={title}>
							{title}
						</span>
					</div>
					<div className="text-muted text-sm">
						{t("record.session.meta", {
							count: row.parts.length,
							duration: formatDuration(row.durationS),
							size: formatSize(row.size),
						})}
					</div>
				</div>
				<Button
					size="sm"
					variant="ghost"
					data-id={`${id}-toggle`}
					aria-expanded={open}
					leadingIcon={
						open ? (
							<IconChevronDown size={16} aria-hidden="true" />
						) : (
							<IconChevronRight size={16} aria-hidden="true" />
						)
					}
					onClick={() => setOpen((value) => !value)}
				>
					{/* Icon only on phones, like the recordings' "Nghe lại". */}
					<span className="max-sm:sr-only">
						{open ? t("record.session.collapse") : t("record.session.expand")}
					</span>
				</Button>
			</div>
			{transcribe ? (
				<TranscriptPanel
					parts={parts}
					requested={row.parts.some(({ item }) => Boolean(item.transcript))}
					name={title}
					session={row.sessionId}
					onSeek={onSeek}
					data-id={`${id}-transcript`}
				/>
			) : null}
			{open ? (
				<ul className="m-0 grid list-none gap-3 border-border-subtle border-l-2 p-0 pl-3">
					{row.parts.map(({ index, item }) => {
						const part = t("record.session.part", {
							number: String(index + 1).padStart(2, "0"),
						});
						const label = `${title} — ${part}`;
						return (
							<RecordingRow
								key={item.id}
								item={item}
								displayName={part}
								retentionDays={retentionDays}
								playing={playingId === item.id}
								seek={seek?.fileId === item.id ? seek.media : null}
								onTogglePlay={() => onTogglePlay(item.id)}
								onDelete={() => onDelete(item, label)}
							/>
						);
					})}
				</ul>
			) : null}
		</li>
	);
}
