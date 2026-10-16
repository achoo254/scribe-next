import { IconBookmark, IconTrash } from "@tabler/icons-react";
import { Card } from "../../components/ui/card";
import { IconButton } from "../../components/ui/icon-button";
import type { Bookmark } from "../../features/transcript-docs/transcript-docs-api";
import { useT } from "../../i18n/i18n-context";
import { formatClock } from "../../lib/format";
import { TimeJump } from "./time-jump";

export interface BookmarkListProps {
	/** Sorted by `at`. */
	bookmarks: readonly Bookmark[];
	/** Id of the bookmark being deleted. */
	deleting: string | null;
	onJump: (at: number) => void;
	onDelete: (bookmark: Bookmark) => void;
}

/** The bookmarks of a transcript in one short list: jump to one, or delete it. */
export function BookmarkList({
	bookmarks,
	deleting,
	onJump,
	onDelete,
}: BookmarkListProps) {
	const t = useT();
	if (bookmarks.length === 0) return null;
	return (
		<Card data-id="reader-bookmarks" className="grid gap-2 p-4 sm:px-6">
			<h2 className="m-0 flex items-center gap-2 font-mono font-semibold text-base">
				<IconBookmark size={18} aria-hidden="true" className="text-primary" />
				{t("transcript.bookmarks.title")}
				<span className="font-normal text-muted text-sm">
					{t("transcript.bookmarks.count", { count: bookmarks.length })}
				</span>
			</h2>
			<ul className="m-0 grid list-none gap-0.5 p-0">
				{bookmarks.map((bookmark, index) => (
					<li
						key={bookmark.id}
						data-id={`reader-bookmark-${bookmark.id}`}
						className="flex items-center gap-2"
					>
						<TimeJump
							at={bookmark.at}
							onJump={onJump}
							data-id={`reader-bookmark-${bookmark.id}-jump`}
						/>
						<span className="min-w-0 flex-1 truncate text-sm">
							{bookmark.label ||
								t("transcript.bookmarks.unnamed", { number: index + 1 })}
						</span>
						<IconButton
							size="sm"
							data-id={`reader-bookmark-${bookmark.id}-delete`}
							label={t("transcript.bookmarks.delete", {
								time: formatClock(bookmark.at),
							})}
							icon={<IconTrash size={16} aria-hidden="true" />}
							loading={deleting === bookmark.id}
							onClick={() => onDelete(bookmark)}
						/>
					</li>
				))}
			</ul>
		</Card>
	);
}
