import { MediaPlayer } from "../../components/media-player";
import { Badge } from "../../components/ui/badge";
import { Button } from "../../components/ui/button";
import type { RecordingItem } from "../../features/files/files-api";
import {
	daysUntilExpiry,
	expiresSoon,
	expiryLabel,
} from "../../features/files/recording-groups";
import { isNativeShell } from "../../lib/api";
import { cn } from "../../lib/cn";
import { downloadPaths, useDownload } from "../../lib/download";
import { formatDateTime, formatDuration, formatSize } from "../../lib/format";

export interface RecordingRowProps {
	item: RecordingItem;
	/** Shown instead of the stored name (iOS segments: "Phần 01"). */
	displayName?: string;
	retentionDays: number;
	playing: boolean;
	onTogglePlay: () => void;
	onDelete: () => void;
}

/** One recording: kind, name, details, actions; the player opens below. */
export function RecordingRow({
	item,
	displayName,
	retentionDays,
	playing,
	onTogglePlay,
	onDelete,
}: RecordingRowProps) {
	const download = useDownload();
	const days = daysUntilExpiry(item.createdAt, retentionDays);
	const expiry = expiryLabel(days);
	const id = `history-item-${item.id}`;

	return (
		<li
			data-id={id}
			className="grid gap-2 border-border-subtle border-t pt-3 first:border-t-0 first:pt-0"
		>
			<div className="flex flex-wrap items-center gap-x-3 gap-y-2">
				<Badge tone={item.hasVideo ? "accent" : "neutral"}>
					{item.hasVideo ? "Màn hình" : "Mic"}
				</Badge>
				<div className="min-w-0 flex-1 basis-48">
					<div
						className="truncate font-semibold"
						title={displayName ?? item.name}
					>
						{displayName ?? item.name}
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
				<div className="flex flex-none flex-wrap items-center gap-1">
					<Button
						size="sm"
						variant="ghost"
						data-id={`${id}-play`}
						aria-expanded={playing}
						onClick={onTogglePlay}
					>
						{playing ? "Ẩn" : "Nghe lại"}
					</Button>
					<Button
						size="sm"
						variant="ghost"
						data-id={`${id}-download`}
						onClick={() => download(downloadPaths.file(item.id))}
					>
						{isNativeShell() ? "Lưu vào Tệp" : "Tải"}
					</Button>
					<Button
						size="sm"
						variant="ghost"
						data-id={`${id}-delete`}
						className="text-error!"
						onClick={onDelete}
					>
						Xoá
					</Button>
				</div>
			</div>
			{playing ? (
				<MediaPlayer
					fileId={item.id}
					video={item.hasVideo}
					durationS={item.durationS}
					data-id={`${id}-player`}
				/>
			) : null}
		</li>
	);
}
