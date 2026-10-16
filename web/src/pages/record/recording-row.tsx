import {
	IconChevronUp,
	IconDownload,
	IconPlayerPlayFilled,
	IconTrash,
} from "@tabler/icons-react";
import { MediaPlayer, type MediaSeek } from "../../components/media-player";
import { Badge } from "../../components/ui/badge";
import { Button } from "../../components/ui/button";
import {
	DropdownMenuItem,
	DropdownMenuSeparator,
} from "../../components/ui/dropdown-menu";
import { RowActionsMenu } from "../../components/ui/row-actions-menu";
import type { RecordingItem } from "../../features/files/files-api";
import {
	daysUntilExpiry,
	expiresSoon,
	expiryLabel,
} from "../../features/files/recording-groups";
import { useT } from "../../i18n/i18n-context";
import { isNativeShell } from "../../lib/api";
import { cn } from "../../lib/cn";
import { downloadPaths, useDownload } from "../../lib/download";
import { formatDateTime, formatDuration, formatSize } from "../../lib/format";
import { TranscriptPanel } from "./transcript-panel";

export interface RecordingRowProps {
	item: RecordingItem;
	/** Shown instead of the stored name (iOS segments: "Phần 01"). */
	displayName?: string;
	retentionDays: number;
	playing: boolean;
	/** Where the player jumps when a transcript sentence is clicked. */
	seek?: MediaSeek | null;
	/** Show "Chép lời" for this recording (single rows; a session has its own). */
	transcribe?: boolean;
	onSeek?: (fileId: string, seconds: number) => void;
	onTogglePlay: () => void;
	onDelete: () => void;
}

/** One recording: kind, name, details, actions; the player opens below. */
export function RecordingRow({
	item,
	displayName,
	retentionDays,
	playing,
	seek,
	transcribe = false,
	onSeek,
	onTogglePlay,
	onDelete,
}: RecordingRowProps) {
	const t = useT();
	const download = useDownload();
	const name = displayName ?? item.name;
	const days = daysUntilExpiry(item.createdAt, retentionDays);
	const expiry = expiryLabel(days);
	const id = `history-item-${item.id}`;

	return (
		<li
			data-id={id}
			className="grid gap-2 border-border-subtle border-t pt-3 first:border-t-0 first:pt-0"
		>
			<div className="flex items-center gap-2">
				{/* Inline-size containment: the long name must not widen the row
				    (and the grids around it) past the screen; it truncates. */}
				<div className="min-w-0 flex-1 contain-inline-size">
					<div className="flex min-w-0 items-center gap-2">
						<Badge tone={item.hasVideo ? "accent" : "neutral"}>
							{item.hasVideo ? t("record.row.screen") : t("record.row.mic")}
						</Badge>
						<span className="truncate font-semibold" title={name}>
							{name}
						</span>
					</div>
					<div className="text-muted text-sm">
						{formatDuration(item.durationS)} · {formatSize(item.size)}
						{item.createdAt ? ` · ${formatDateTime(item.createdAt)}` : ""}
						{expiry ? (
							<span
								className={cn(
									expiresSoon(days) && "font-semibold text-warning-text",
								)}
							>
								{` · ${expiry}`}
							</span>
						) : null}
					</div>
				</div>
				<div className="flex flex-none items-center gap-1">
					<Button
						size="sm"
						variant="ghost"
						data-id={`${id}-play`}
						aria-expanded={playing}
						leadingIcon={
							playing ? (
								<IconChevronUp size={16} aria-hidden="true" />
							) : (
								<IconPlayerPlayFilled size={16} aria-hidden="true" />
							)
						}
						onClick={onTogglePlay}
					>
						{/* Icon only on phones: the name keeps the room; still the button's name. */}
						<span className="max-sm:sr-only">
							{playing ? t("record.row.hide") : t("record.row.play")}
						</span>
					</Button>
					<RowActionsMenu
						label={t("record.row.menu", { name })}
						data-id={`${id}-menu`}
					>
						<DropdownMenuItem
							data-id={`${id}-download`}
							icon={<IconDownload size={16} aria-hidden="true" />}
							onSelect={() => download(downloadPaths.file(item.id))}
						>
							{isNativeShell()
								? t("record.row.saveToFiles")
								: t("record.row.download")}
						</DropdownMenuItem>
						<DropdownMenuSeparator />
						<DropdownMenuItem
							danger
							data-id={`${id}-delete`}
							icon={<IconTrash size={16} aria-hidden="true" />}
							onSelect={onDelete}
						>
							{t("common.actions.delete")}
						</DropdownMenuItem>
					</RowActionsMenu>
				</div>
			</div>
			{playing ? (
				<MediaPlayer
					fileId={item.id}
					video={item.hasVideo}
					durationS={item.durationS}
					seek={seek}
					data-id={`${id}-player`}
				/>
			) : null}
			{transcribe && onSeek ? (
				<TranscriptPanel
					parts={[{ fileId: item.id, durationS: item.durationS }]}
					requested={Boolean(item.transcript)}
					name={item.name}
					onSeek={onSeek}
					data-id={`${id}-transcript`}
				/>
			) : null}
		</li>
	);
}
