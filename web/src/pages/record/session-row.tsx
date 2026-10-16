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
			<div className="flex flex-wrap items-center gap-x-3 gap-y-2">
				<Badge tone="primary">iPhone</Badge>
				<div className="min-w-0 flex-1 basis-48">
					<div className="truncate font-semibold">{title}</div>
					<div className="text-muted text-sm">
						{row.parts.length} đoạn · {formatDuration(row.durationS)} ·{" "}
						{formatSize(row.size)}
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
					{open ? "Thu gọn" : "Xem các đoạn"}
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
						const label = `${title} — Phần ${String(index + 1).padStart(2, "0")}`;
						return (
							<RecordingRow
								key={item.id}
								item={item}
								displayName={`Phần ${String(index + 1).padStart(2, "0")}`}
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
