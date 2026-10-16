import { IconFileText, IconPencil, IconTrash } from "@tabler/icons-react";
import { useQueryClient } from "@tanstack/react-query";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { AudioPlayer, type MediaSeek } from "../../components/media-player";
import { Alert } from "../../components/ui/alert";
import { Button } from "../../components/ui/button";
import { Card } from "../../components/ui/card";
import { ConfirmModal } from "../../components/ui/confirm-modal";
import {
	DropdownMenuItem,
	DropdownMenuSeparator,
} from "../../components/ui/dropdown-menu";
import { IconButton } from "../../components/ui/icon-button";
import { Input } from "../../components/ui/input";
import { LoadError } from "../../components/ui/load-error";
import { RowActionsMenu } from "../../components/ui/row-actions-menu";
import { Skeleton, SkeletonRegion } from "../../components/ui/skeleton";
import { useToast } from "../../components/ui/toast";
import {
	activeSentence,
	bookmarksBySegment,
	gapsBefore,
	isPending,
	nextSegment,
	overallProgress,
	placeOf,
	sentenceAt,
	shouldPreload,
	transcriptTitle,
	untranscribed,
} from "../../features/transcript-docs/reader-model";
import type {
	Bookmark,
	TranscriptDetail,
	TranscriptSegment,
} from "../../features/transcript-docs/transcript-docs-api";
import {
	transcriptDocKeys,
	useDeleteBookmark,
	useDeleteTranscript,
	useRenameTranscript,
	useSaveSegment,
	useTranscriptDoc,
} from "../../features/transcript-docs/transcript-docs-queries";
import type { ExportScope } from "../../features/transcript/transcript-api";
import { useT } from "../../i18n/i18n-context";
import { LeaveGuard } from "../../layout/leave-guard";
import { PageHeader } from "../../layout/page-header";
import { errorMessage, isNativeShell } from "../../lib/api";
import {
	downloadPaths,
	preloadMedia,
	useDownload,
	useMediaSource,
} from "../../lib/download";
import { formatDateTime, formatDuration } from "../../lib/format";
import { useDelayedFlag } from "../../lib/use-delayed-flag";
import { useTranscribe } from "../record/use-transcribe";
import { AskCard } from "../search/ask-card";
import { BookmarkList } from "./bookmark-list";
import { MinutesCard } from "./minutes-card";
import { ReaderSegment } from "./reader-segment";

export interface TranscriptReaderProps {
	transcriptKey: string;
	/** Seconds of the session to show and play once loaded (`?t=` of a search result). */
	startAt?: number;
	canTranscribe: boolean;
	/** `replace` after a delete, so back/swipe does not reopen the deleted doc. */
	onBack: (options?: { replace?: boolean }) => void;
}

interface Playback {
	fileId: string;
	seek: MediaSeek | null;
	autoPlay: boolean;
}

const NO_BOOKMARKS: Bookmark[] = [];

/**
 * Brings the sentence at `seconds` of `segment` into view and focuses it
 * (the segment itself when it has no sentences yet).
 */
function revealSentence(segment: TranscriptSegment, seconds: number): void {
	const index = sentenceAt(segment, seconds);
	const id =
		index >= 0
			? `reader-segment-${segment.index}-line-${index}`
			: `reader-segment-${segment.index}`;
	const target = document.querySelector<HTMLElement>(`[data-id="${id}"]`);
	if (!target) return;
	const reduce = window.matchMedia?.(
		"(prefers-reduced-motion: reduce)",
	).matches;
	if (typeof target.scrollIntoView === "function") {
		target.scrollIntoView({
			block: "center",
			behavior: reduce ? "auto" : "smooth",
		});
	}
	target.focus({ preventScroll: true });
}

/** What `.txt`/`.srt` export covers: the whole session, or the one file. */
function exportScope(detail: TranscriptDetail): ExportScope {
	if (detail.key.startsWith("s:")) return { session: detail.key.slice(2) };
	return { fileIds: detail.segments.map((s) => s.fileId) };
}

/**
 * Reads one transcript: sentences of every segment in order, a tap plays the
 * audio from there and playback carries on into the next segment; rename,
 * edit sentences per segment, transcribe again, export, delete.
 */
export function TranscriptReader({
	transcriptKey,
	startAt,
	canTranscribe,
	onBack,
}: TranscriptReaderProps) {
	const t = useT();
	const toast = useToast();
	const queryClient = useQueryClient();
	const download = useDownload();
	const { detail, statuses } = useTranscriptDoc(transcriptKey);
	const skeleton = useDelayedFlag(detail.isPending);
	const rename = useRenameTranscript();
	const save = useSaveSegment();
	const remove = useDeleteTranscript();
	const removeBookmark = useDeleteBookmark(transcriptKey);
	const transcribe = useTranscribe(() => {
		void queryClient.invalidateQueries({ queryKey: transcriptDocKeys.all });
	});

	const [playback, setPlayback] = useState<Playback | null>(null);
	const [time, setTime] = useState(0);
	const preloaded = useRef(new Set<string>());
	const [editing, setEditing] = useState<string | null>(null);
	const [titleDraft, setTitleDraft] = useState<string | null>(null);
	const [titleError, setTitleError] = useState("");
	const [confirmDelete, setConfirmDelete] = useState(false);
	const [rerun, setRerun] = useState<TranscriptSegment | null>(null);
	// Deleted: open editors no longer matter, so leaving is not guarded.
	// Navigating from an effect lets `LeaveGuard` (a child, whose effects
	// run first) drop its blocker before the navigation starts.
	const [removed, setRemoved] = useState(false);
	useEffect(() => {
		if (removed) onBack({ replace: true });
	}, [removed, onBack]);
	const unsaved = !removed && (editing !== null || titleDraft !== null);

	const doc = detail.data;
	const segments = doc?.segments ?? [];
	const current =
		segments.find((s) => s.fileId === playback?.fileId) ?? segments[0];
	const gaps = useMemo(
		() => (doc ? gapsBefore(doc) : new Map<string, number[]>()),
		[doc],
	);
	const live = useMemo(
		() => new Map((statuses ?? []).map((s) => [s.fileId, s])),
		[statuses],
	);
	const bookmarks = doc?.bookmarks ?? NO_BOOKMARKS;
	const marks = useMemo(
		() => bookmarksBySegment(segments, bookmarks),
		[segments, bookmarks],
	);
	const toSend = untranscribed(segments);
	const pending = segments.filter((s) => isPending(s.status)).length;

	const play = useCallback((fileId: string, seconds: number) => {
		setPlayback({ fileId, seek: { time: seconds }, autoPlay: false });
		setTime(seconds);
	}, []);

	/** A time of the minutes or a bookmark (session seconds): show and play it. */
	const jumpTo = useCallback(
		(at: number) => {
			const place = placeOf(segments, at);
			if (!place) return;
			play(place.segment.fileId, place.seconds);
			revealSentence(place.segment, place.seconds);
		},
		[segments, play],
	);

	// Opened at a time (search result, source of an answer): jump as soon as
	// the sentences are on screen, once per time asked for — a new `?t=` on
	// the same transcript (back/forward) jumps again without a remount.
	const jumpedTo = useRef<number | undefined>(undefined);
	useEffect(() => {
		if (startAt === undefined) {
			jumpedTo.current = undefined;
			return;
		}
		if (jumpedTo.current === startAt || segments.length === 0) return;
		jumpedTo.current = startAt;
		jumpTo(startAt);
	}, [startAt, segments.length, jumpTo]);

	function deleteBookmark(bookmark: Bookmark): void {
		removeBookmark.mutate(bookmark.id, {
			onSuccess: () => toast.show({ title: t("transcript.bookmarks.deleted") }),
			onError: (cause) =>
				toast.show({
					tone: "error",
					title: t("transcript.bookmarks.deleteFailed"),
					description: errorMessage(cause),
				}),
		});
	}

	const onEditingChange = useCallback((fileId: string, on: boolean) => {
		setEditing((value) => (on ? fileId : value === fileId ? null : value));
	}, []);

	const saveSegment = useCallback(
		async (fileId: string, runId: string, texts: string[]) => {
			await save.mutateAsync({ key: transcriptKey, fileId, runId, texts });
		},
		[save.mutateAsync, transcriptKey],
	);

	// Segments are memoized; the callbacks they get must not change per render.
	const startTranscribe = useRef(transcribe.start);
	startTranscribe.current = transcribe.start;
	const requestRerun = useCallback((segment: TranscriptSegment) => {
		if (segment.edited) setRerun(segment);
		else void startTranscribe.current([segment.fileId], segment.durationS);
	}, []);

	function onTime(seconds: number): void {
		setTime(seconds);
		if (!current) return;
		const next = nextSegment(segments, current.fileId);
		if (
			next &&
			!preloaded.current.has(next.fileId) &&
			shouldPreload(seconds, current.durationS)
		) {
			preloaded.current.add(next.fileId);
			preloadMedia(next.fileId);
		}
	}

	function onEnded(): void {
		if (!current) return;
		const next = nextSegment(segments, current.fileId);
		if (!next) return;
		setPlayback({ fileId: next.fileId, seek: null, autoPlay: true });
		setTime(0);
	}

	async function submitTitle(): Promise<void> {
		if (titleDraft === null) return;
		setTitleError("");
		try {
			await rename.mutateAsync({ key: transcriptKey, title: titleDraft });
			setTitleDraft(null);
			toast.show({ title: t("transcript.reader.renamed") });
		} catch (cause) {
			setTitleError(errorMessage(cause));
		}
	}

	if (skeleton) return <ReaderSkeleton />;
	if (detail.isPending) return null;
	if (detail.isError || !doc) {
		return (
			<section data-id="reader" className="grid gap-5">
				<PageHeader
					parent={<BackLink onBack={onBack} />}
					title={t("transcript.reader.errorTitle")}
				/>
				<LoadError
					data-id="reader-error"
					title={t("transcript.reader.loadFailed")}
					error={detail.error}
					retrying={detail.isFetching}
					onRetry={() => void detail.refetch()}
				/>
			</section>
		);
	}

	const title = transcriptTitle(doc);
	const totalS = segments.reduce((sum, s) => sum + s.durationS, 0);
	const anyDone = segments.some((s) => s.status === "done");
	const scope = exportScope(doc);

	return (
		<section data-id="reader" className="grid gap-5">
			<PageHeader
				parent={<BackLink onBack={onBack} />}
				title={title}
				description={
					<>
						{formatDateTime(doc.createdAt ?? undefined)} ·{" "}
						{formatDuration(totalS)}
						{segments.length > 1
							? ` · ${t("transcript.list.segments", { count: segments.length })}`
							: ""}
					</>
				}
				actions={
					<>
						{titleDraft === null ? (
							<IconButton
								label={t("transcript.reader.rename")}
								data-id="reader-rename"
								icon={<IconPencil size={20} aria-hidden="true" />}
								onClick={() => setTitleDraft(doc.title ?? "")}
							/>
						) : null}
						<RowActionsMenu
							label={t("transcript.reader.menu")}
							data-id="reader-menu"
						>
							{anyDone ? (
								<>
									<DropdownMenuItem
										data-id="reader-txt"
										icon={<IconFileText size={16} aria-hidden="true" />}
										onSelect={() =>
											download(downloadPaths.transcript(scope, "txt", title))
										}
									>
										{isNativeShell()
											? t("transcript.reader.saveTxt")
											: t("transcript.reader.downloadTxt")}
									</DropdownMenuItem>
									<DropdownMenuItem
										data-id="reader-srt"
										icon={<IconFileText size={16} aria-hidden="true" />}
										onSelect={() =>
											download(downloadPaths.transcript(scope, "srt", title))
										}
									>
										{isNativeShell()
											? t("transcript.reader.saveSrt")
											: t("transcript.reader.downloadSrt")}
									</DropdownMenuItem>
									<DropdownMenuSeparator />
								</>
							) : null}
							<DropdownMenuItem
								danger
								data-id="reader-delete"
								icon={<IconTrash size={16} aria-hidden="true" />}
								onSelect={() => setConfirmDelete(true)}
							>
								{t("transcript.reader.delete")}
							</DropdownMenuItem>
						</RowActionsMenu>
					</>
				}
			/>

			{titleDraft !== null ? (
				<Card className="p-4 sm:p-6">
					<form
						className="grid gap-2"
						onSubmit={(event) => {
							event.preventDefault();
							void submitTitle();
						}}
					>
						<Input
							data-id="reader-title-input"
							aria-label={t("transcript.reader.titleField")}
							placeholder={transcriptTitle({ ...doc, title: null })}
							value={titleDraft}
							maxLength={120}
							autoFocus
							onChange={(event) => setTitleDraft(event.target.value)}
						/>
						{titleError ? <Alert tone="danger">{titleError}</Alert> : null}
						<div className="flex flex-wrap items-center justify-between gap-2">
							<p className="m-0 text-muted text-sm">
								{t("transcript.reader.titleHint")}
							</p>
							<div className="flex flex-wrap gap-2">
								<Button
									variant="ghost"
									data-id="reader-title-cancel"
									disabled={rename.isPending}
									onClick={() => {
										setTitleDraft(null);
										setTitleError("");
									}}
								>
									{t("common.actions.cancel")}
								</Button>
								<Button
									type="submit"
									variant="primary"
									data-id="reader-title-save"
									loading={rename.isPending}
								>
									{t("transcript.reader.saveTitle")}
								</Button>
							</div>
						</div>
					</form>
				</Card>
			) : null}

			{canTranscribe ? (
				<MinutesCard
					transcriptKey={transcriptKey}
					summary={doc.summary ?? null}
					canSummarize={anyDone}
					onJump={jumpTo}
				/>
			) : null}
			{canTranscribe && anyDone ? (
				<AskCard
					transcriptKey={transcriptKey}
					onCite={(citation) => jumpTo(citation.at)}
				/>
			) : null}
			<BookmarkList
				bookmarks={bookmarks}
				deleting={
					removeBookmark.isPending ? (removeBookmark.variables ?? null) : null
				}
				onJump={jumpTo}
				onDelete={deleteBookmark}
			/>

			<Card data-id="reader-body" className="grid gap-4 p-4 sm:p-6">
				{pending > 0 ? (
					<Alert tone="info" data-id="reader-progress">
						{t("transcript.progress", {
							percent: overallProgress(segments, statuses),
						})}
					</Alert>
				) : null}
				{canTranscribe && pending === 0 && toSend.length > 0 ? (
					<Alert tone="warning" data-id="reader-unsent">
						{t("transcript.reader.unsent", { count: toSend.length })}
						<Button
							size="sm"
							variant="ghost"
							className="ml-2"
							data-id="reader-send-rest"
							loading={transcribe.pending}
							onClick={() =>
								void transcribe.start(
									toSend.map((s) => s.fileId),
									toSend.reduce((sum, s) => sum + s.durationS, 0),
								)
							}
						>
							{t("transcript.reader.sendRest", { count: toSend.length })}
						</Button>
					</Alert>
				) : null}

				<div className="grid gap-4">
					{segments.map((segment) => (
						<div key={segment.fileId} className="grid gap-1">
							{(gaps.get(segment.fileId) ?? []).map((missing) => (
								<p
									key={missing}
									data-id={`reader-missing-${missing}`}
									className="m-0 rounded-sm border border-border border-dashed px-2 py-2 text-muted text-sm"
								>
									{t("transcript.reader.missing", { number: missing + 1 })}
								</p>
							))}
							<ReaderSegment
								segment={segment}
								numbered={transcriptKey.startsWith("s:")}
								progress={
									live.get(segment.fileId)?.progress ?? segment.progress
								}
								activeIndex={
									segment.fileId === current?.fileId && playback
										? activeSentence(segment, time)
										: -1
								}
								editLocked={editing !== null && editing !== segment.fileId}
								canTranscribe={canTranscribe}
								bookmarks={marks.get(segment.fileId)}
								onPlay={play}
								onEditingChange={onEditingChange}
								onSave={saveSegment}
								onRerun={requestRerun}
							/>
						</div>
					))}
				</div>

				{current ? (
					<div className="sticky bottom-0 z-10 -mx-4 border-border-subtle border-t bg-surface px-4 pt-2 pb-[max(var(--space-2),env(safe-area-inset-bottom))] sm:-mx-6 sm:px-6">
						{segments.length > 1 ? (
							<div className="pb-1 font-mono text-muted text-xs">
								{t("transcript.reader.playing", {
									number: current.index + 1,
								})}
							</div>
						) : null}
						<ReaderPlayer
							key={current.fileId}
							segment={current}
							seek={playback?.fileId === current.fileId ? playback.seek : null}
							autoPlay={
								playback?.fileId === current.fileId ? playback.autoPlay : false
							}
							onTime={onTime}
							onEnded={onEnded}
						/>
					</div>
				) : null}
			</Card>

			{transcribe.dialog}
			<LeaveGuard when={unsaved} data-id="reader-leave" />
			<ConfirmModal
				open={rerun !== null}
				onOpenChange={(next) => {
					if (!next) setRerun(null);
				}}
				title={t("transcript.reader.rerunTitle")}
				question={t("transcript.reader.rerunQuestion", {
					count: rerun?.editedSentences ?? 0,
					number: (rerun?.index ?? 0) + 1,
				})}
				confirmLabel={t("transcript.panel.rerun")}
				onConfirm={() =>
					transcribe.start(rerun ? [rerun.fileId] : [], rerun?.durationS ?? 0)
				}
				data-id="reader-rerun"
			>
				{t("transcript.reader.rerunBody")}
			</ConfirmModal>
			<ConfirmModal
				open={confirmDelete}
				onOpenChange={setConfirmDelete}
				title={t("transcript.reader.deleteTitle")}
				question={t("transcript.reader.deleteQuestion", { name: title })}
				confirmLabel={t("common.actions.delete")}
				onConfirm={async () => {
					await remove.mutateAsync(transcriptKey);
					toast.show({
						title: t("transcript.reader.deleted", { name: title }),
					});
					setRemoved(true);
				}}
				data-id="reader-delete-confirm"
			>
				{t("transcript.reader.deleteBody")}
			</ConfirmModal>
		</section>
	);
}

/** Start of the breadcrumb: back to the list, like a parent page link. */
function BackLink({ onBack }: { onBack: () => void }) {
	const t = useT();
	return (
		<button
			type="button"
			data-id="reader-back"
			className="shrink-0 cursor-pointer rounded-sm border-0 bg-transparent p-0 font-bold font-mono text-muted text-xl hover:text-primary pointer-coarse:min-h-11"
			onClick={() => onBack()}
		>
			{t("transcript.reader.back")}
		</button>
	);
}

/** First load: header, meta line and a card of sentences. */
function ReaderSkeleton() {
	const t = useT();
	return (
		<SkeletonRegion
			data-id="reader-loading"
			label={t("transcript.reader.loading")}
			className="grid gap-5"
		>
			<div className="grid gap-2">
				<Skeleton className="h-7 w-72 max-w-full" />
				<Skeleton className="h-3 w-48 max-w-full" />
			</div>
			<Card className="grid gap-4 p-4 sm:p-6">
				<Skeleton className="h-4 w-40 max-w-full" />
				<Skeleton variant="text" lines={4} />
				<Skeleton variant="text" lines={3} />
			</Card>
		</SkeletonRegion>
	);
}

interface ReaderPlayerProps {
	segment: TranscriptSegment;
	seek: MediaSeek | null;
	autoPlay: boolean;
	onTime: (seconds: number) => void;
	onEnded: () => void;
}

/** Player of the segment being heard; remounted per segment (`key`). */
function ReaderPlayer({
	segment,
	seek,
	autoPlay,
	onTime,
	onEnded,
}: ReaderPlayerProps) {
	const t = useT();
	const { src, error } = useMediaSource(segment.fileId);
	if (error) {
		return (
			<p role="alert" className="m-0 text-error text-sm">
				{t("ui.player.playFailed", { error })}
			</p>
		);
	}
	if (!src) {
		return (
			<p aria-live="polite" className="m-0 text-muted text-sm">
				{t("ui.player.loadingAudio")}
			</p>
		);
	}
	return (
		<AudioPlayer
			src={src}
			durationS={segment.durationS}
			seek={seek}
			autoPlay={autoPlay}
			onTimeUpdate={onTime}
			onEnded={onEnded}
			data-id="reader-player"
		/>
	);
}
