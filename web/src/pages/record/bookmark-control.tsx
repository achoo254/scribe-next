import { IconBookmark } from "@tabler/icons-react";
import { Button } from "../../components/ui/button";
import type { BookmarkQueueState } from "../../features/transcript-docs/bookmark-queue";
import { useT } from "../../i18n/i18n-context";

export interface BookmarkControlProps {
	/** Null when the recording cannot take bookmarks yet. */
	onMark: (() => void) | null;
	bookmarks: BookmarkQueueState;
	/** Prefix of the `data-id`s (`record`, `native-record`). */
	idPrefix: string;
}

/**
 * "Đánh dấu" beside the stop button, browser and iOS alike, with a short
 * line on what was marked. Saving runs in the background (BookmarkQueue):
 * a failure is only said here, the recording goes on.
 */
export function BookmarkControl({
	onMark,
	bookmarks,
	idPrefix,
}: BookmarkControlProps) {
	const t = useT();
	const { marked, waiting, failed, dropped, lastError } = bookmarks;
	const parts = [
		marked > 0 ? t("record.bookmark.marked", { count: marked }) : "",
		waiting > 0 ? t("record.bookmark.waiting", { count: waiting }) : "",
		dropped > 0 ? t("record.bookmark.dropped", { count: dropped }) : "",
	].filter(Boolean);
	return (
		<>
			<Button
				data-id={`${idPrefix}-bookmark`}
				disabled={onMark === null}
				leadingIcon={<IconBookmark size={18} aria-hidden="true" />}
				onClick={() => onMark?.()}
			>
				{t("record.bookmark.mark")}
			</Button>
			<p
				data-id={`${idPrefix}-bookmarks`}
				aria-live="polite"
				className="m-0 basis-full text-muted text-sm empty:hidden"
			>
				{parts.join(" · ")}
				{failed > 0 ? (
					<span className="block text-warning-text">
						{t("record.bookmark.failed", {
							count: failed,
							reason: lastError || t("record.bookmark.unknownReason"),
						})}
					</span>
				) : null}
			</p>
		</>
	);
}
