import { IconBookmark, IconPencil, IconRefresh } from "@tabler/icons-react";
import { memo, useState } from "react";
import { Alert } from "../../components/ui/alert";
import { Button } from "../../components/ui/button";
import { controlClassName } from "../../components/ui/input";
import {
	isPending,
	sentenceAt,
} from "../../features/transcript-docs/reader-model";
import type {
	Bookmark,
	TranscriptSegment,
} from "../../features/transcript-docs/transcript-docs-api";
import { useT } from "../../i18n/i18n-context";
import type { Translate } from "../../i18n/translate";
import { errorMessage } from "../../lib/api";
import { cn } from "../../lib/cn";
import { formatClock } from "../../lib/format";

export interface ReaderSegmentProps {
	segment: TranscriptSegment;
	/** Show "Đoạn N" headers (iPhone sessions have many segments). */
	numbered: boolean;
	/** Live progress while running (from the status poll). */
	progress: number;
	/** Sentence being heard; -1 when this segment is not playing. */
	activeIndex: number;
	/** Another segment is being edited: one at a time. */
	editLocked: boolean;
	canTranscribe: boolean;
	/** Bookmarks inside this segment (session times), in order. */
	bookmarks?: readonly Bookmark[];
	onPlay: (fileId: string, seconds: number) => void;
	onEditingChange: (fileId: string, editing: boolean) => void;
	/** Saves against `runId`, the run the draft was started on. */
	onSave: (fileId: string, runId: string, texts: string[]) => Promise<void>;
	onRerun: (segment: TranscriptSegment) => void;
}

const NO_BOOKMARKS: readonly Bookmark[] = [];

/** "Dấu mốc 1:23 · label" beside the sentence it was pressed during. */
function BookmarkMark({ bookmark }: { bookmark: Bookmark }) {
	const t = useT();
	const time = formatClock(bookmark.at);
	return (
		<p
			data-id={`reader-mark-${bookmark.id}`}
			className="m-0 flex items-center gap-1.5 px-2 pt-1 font-mono text-primary text-xs"
		>
			<IconBookmark size={14} aria-hidden="true" />
			{bookmark.label
				? t("transcript.bookmarks.inlineLabel", { time, label: bookmark.label })
				: t("transcript.bookmarks.inline", { time })}
		</p>
	);
}

/** Rows of a textarea for a sentence, so short ones stay one line. */
function rowsFor(text: string): number {
	return Math.min(6, Math.max(1, Math.ceil(text.length / 42)));
}

function statusLine(
	t: Translate,
	segment: TranscriptSegment,
	progress: number,
): string {
	switch (segment.status) {
		case "none":
			return t("transcript.segment.none");
		case "queued":
			return t("transcript.segment.queued");
		case "running":
			return t("transcript.segment.running", {
				percent: Math.floor(progress),
			});
		case "failed":
			return t("transcript.segment.failed", {
				reason: segment.error || t("transcript.segment.unknownReason"),
			});
		default:
			return t("transcript.segment.empty");
	}
}

/**
 * One segment of a transcript: its sentences (tap to play from there), or
 * a status line while it is not transcribed; "Sửa" turns the sentences
 * into text fields saved together.
 */
export const ReaderSegment = memo(function ReaderSegment({
	segment,
	numbered,
	progress,
	activeIndex,
	editLocked,
	canTranscribe,
	bookmarks = NO_BOOKMARKS,
	onPlay,
	onEditingChange,
	onSave,
	onRerun,
}: ReaderSegmentProps) {
	const t = useT();
	const [drafts, setDrafts] = useState<string[] | null>(null);
	// The run the draft was made from. If the segment is transcribed again
	// meanwhile, these texts belong to other sentences and must not be saved.
	const [draftRun, setDraftRun] = useState<string | null>(null);
	const [saving, setSaving] = useState(false);
	const [error, setError] = useState("");
	const id = `reader-segment-${segment.index}`;
	const done = segment.status === "done";
	const editable = done && segment.sentences.length > 0 && segment.runId;
	const busy = isPending(segment.status);
	const stale = drafts !== null && draftRun !== segment.runId;

	function startEdit(): void {
		setDrafts(segment.sentences.map((s) => s.text));
		setDraftRun(segment.runId);
		setError("");
		onEditingChange(segment.fileId, true);
	}

	function stopEdit(): void {
		setDrafts(null);
		setDraftRun(null);
		setError("");
		onEditingChange(segment.fileId, false);
	}

	async function save(): Promise<void> {
		if (!drafts || !draftRun || stale) return;
		setSaving(true);
		setError("");
		try {
			await onSave(segment.fileId, draftRun, drafts);
			stopEdit();
		} catch (cause) {
			setError(errorMessage(cause));
		} finally {
			setSaving(false);
		}
	}

	return (
		<section data-id={id} className="grid gap-1">
			<div className="flex flex-wrap items-center gap-x-2 gap-y-1">
				{numbered ? (
					<h3 className="m-0 mr-auto font-mono font-semibold text-muted text-[0.8125rem]">
						{t("transcript.segment.heading", {
							number: segment.index + 1,
							offset: formatClock(segment.offsetS),
						})}
						{segment.edited ? t("transcript.segment.editedSuffix") : ""}
					</h3>
				) : (
					<span className="mr-auto font-mono text-muted text-[0.8125rem]">
						{segment.edited ? t("transcript.segment.edited") : ""}
					</span>
				)}
				{drafts ? null : (
					<>
						{editable ? (
							<Button
								size="sm"
								variant="ghost"
								data-id={`${id}-edit`}
								disabled={editLocked}
								leadingIcon={<IconPencil size={16} aria-hidden="true" />}
								onClick={startEdit}
							>
								{t("common.actions.edit")}
							</Button>
						) : null}
						{canTranscribe && !busy && segment.status !== "none" ? (
							<Button
								size="sm"
								variant="ghost"
								data-id={`${id}-rerun`}
								disabled={editLocked}
								leadingIcon={<IconRefresh size={16} aria-hidden="true" />}
								onClick={() => onRerun(segment)}
							>
								{t("transcript.segment.rerun")}
							</Button>
						) : null}
					</>
				)}
			</div>

			{drafts ? (
				<div className="grid gap-2">
					{drafts.map((text, index) => {
						const sentence = segment.sentences[index];
						return (
							<label
								// Sentences of one run never reorder: the index is their identity.
								// biome-ignore lint/suspicious/noArrayIndexKey: fixed-order list
								key={index}
								className="grid grid-cols-[auto_1fr] items-start gap-3"
							>
								<span className="pt-2.5 font-mono text-muted text-sm tabular-nums">
									{formatClock(segment.offsetS + (sentence?.start ?? 0))}
								</span>
								<textarea
									data-id={`${id}-draft-${index}`}
									aria-label={t("transcript.segment.sentence", {
										number: index + 1,
									})}
									value={text}
									rows={rowsFor(text)}
									maxLength={2000}
									onChange={(event) => {
										const next = [...drafts];
										next[index] = event.target.value;
										setDrafts(next);
									}}
									className={cn(controlClassName, "resize-y px-3 py-2")}
								/>
							</label>
						);
					})}
					{stale ? (
						<Alert tone="warning" data-id={`${id}-stale`}>
							{t("transcript.segment.stale")}
						</Alert>
					) : error ? (
						<Alert tone="danger" data-id={`${id}-error`}>
							{error}
						</Alert>
					) : null}
					<div className="flex flex-wrap justify-end gap-2">
						<Button
							variant="ghost"
							data-id={`${id}-cancel`}
							disabled={saving}
							onClick={stopEdit}
						>
							{t("common.actions.cancel")}
						</Button>
						<Button
							variant="primary"
							data-id={`${id}-save`}
							loading={saving}
							disabled={stale}
							onClick={() => void save()}
						>
							{t("common.actions.save")}
						</Button>
					</div>
				</div>
			) : done && segment.sentences.length > 0 ? (
				<ol className="m-0 grid list-none gap-0.5 p-0">
					{segment.sentences.map((sentence, index) => (
						<li
							// biome-ignore lint/suspicious/noArrayIndexKey: fixed-order list
							key={index}
						>
							{bookmarks
								.filter(
									(b) => sentenceAt(segment, b.at - segment.offsetS) === index,
								)
								.map((b) => (
									<BookmarkMark key={b.id} bookmark={b} />
								))}
							<button
								type="button"
								data-id={`${id}-line-${index}`}
								aria-current={index === activeIndex ? "true" : undefined}
								onClick={() => onPlay(segment.fileId, sentence.start)}
								className={cn(
									"flex min-h-11 w-full cursor-pointer gap-3 rounded-sm px-2 py-2 text-left hover:bg-surface-subtle focus-visible:shadow-focus-primary focus-visible:outline-none",
									index === activeIndex && "bg-primary/10",
								)}
							>
								<span className="flex-none pt-0.5 font-mono text-muted text-sm tabular-nums">
									{formatClock(segment.offsetS + sentence.start)}
								</span>
								<span>{sentence.text}</span>
							</button>
						</li>
					))}
				</ol>
			) : (
				<>
					{bookmarks.map((b) => (
						<BookmarkMark key={b.id} bookmark={b} />
					))}
					<button
						type="button"
						data-id={`${id}-status`}
						onClick={() => onPlay(segment.fileId, 0)}
						className={cn(
							"flex min-h-11 w-full cursor-pointer gap-3 rounded-sm px-2 py-2 text-left text-sm hover:bg-surface-subtle focus-visible:shadow-focus-primary focus-visible:outline-none",
							segment.status === "failed" ? "text-error-text" : "text-muted",
						)}
					>
						<span className="flex-none font-mono tabular-nums">
							{formatClock(segment.offsetS)}
						</span>
						<span aria-live={busy ? "polite" : undefined}>
							{statusLine(t, segment, progress)}
						</span>
					</button>
				</>
			)}
		</section>
	);
});
