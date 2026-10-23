import { IconPencil, IconTrash, IconUsers } from "@tabler/icons-react";
import { useQueryClient } from "@tanstack/react-query";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useAuth } from "../../auth/auth-context";
import { AudioPlayer, type MediaSeek } from "../../components/media-player";
import { TranscriptDownloadMenu } from "../../components/transcript-download-menu";
import { Alert } from "../../components/ui/alert";
import { Button } from "../../components/ui/button";
import { Card } from "../../components/ui/card";
import { ConfirmModal } from "../../components/ui/confirm-modal";
import { DropdownMenuItem } from "../../components/ui/dropdown-menu";
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
	renamedSpeakers,
	sentenceAt,
	shouldPreload,
	speakersBefore,
	transcriptTitle,
	untranscribed,
} from "../../features/transcript-docs/reader-model";
import {
	type Bookmark,
	MAX_TITLE_CHARS,
	type TranscriptDetail,
	type TranscriptSegment,
	type TranslationLang,
	type TranslationState,
} from "../../features/transcript-docs/transcript-docs-api";
import {
	transcriptDocKeys,
	useDeleteBookmark,
	useDeleteTranscript,
	useRenameSpeakers,
	useRenameTranscript,
	useSaveSegment,
	useTranscriptDoc,
	useTranslation,
} from "../../features/transcript-docs/transcript-docs-queries";
import type { ExportScope } from "../../features/transcript/transcript-api";
import { useT } from "../../i18n/i18n-context";
import { LeaveGuard } from "../../layout/leave-guard";
import { PageHeader } from "../../layout/page-header";
import { errorMessage } from "../../lib/api";
import { preloadMedia, useMediaSource } from "../../lib/download";
import { formatDateTime, formatDuration } from "../../lib/format";
import { useDelayedFlag } from "../../lib/use-delayed-flag";
import { useTranscribe } from "../record/use-transcribe";
import { AskCard } from "../search/ask-card";
import { BookmarkList } from "./bookmark-list";
import { MinutesCard } from "./minutes-card";
import { ReaderSegment } from "./reader-segment";
import { SpeakerNamesDialog } from "./speaker-names-dialog";
import {
	TranslationControls,
	type TranslationView,
} from "./translation-controls";

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
const NO_TRANSLATIONS: TranslationState[] = [];
const NO_SPEAKERS: Readonly<Record<string, string>> = {};

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
	const { detail, statuses } = useTranscriptDoc(transcriptKey);
	const skeleton = useDelayedFlag(detail.isPending);
	const { user } = useAuth();
	const rename = useRenameTranscript();
	const renameSpeakers = useRenameSpeakers();
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
	/** What "Chép lại cả phiên" would drop (edited sentences, renamed speakers); null = not asking. */
	const [rerunAll, setRerunAll] = useState<{
		edited: number;
		renamed: number;
	} | null>(null);
	const [speakersOpen, setSpeakersOpen] = useState(false);
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
	const available = doc?.translations ?? NO_TRANSLATIONS;
	const [chosenLang, setChosenLang] = useState<TranslationLang | null>(null);
	const [view, setView] = useState<TranslationView>("original");
	const lang = chosenLang ?? available[0]?.lang ?? null;
	const { translation, progress } = useTranslation(transcriptKey, lang);
	const translated = useMemo(
		() =>
			new Map(
				(translation?.segments ?? []).map((s) => [s.fileId, s.sentences]),
			),
		[translation],
	);
	const shownView: TranslationView =
		translation && translation.status !== "none" ? view : "original";
	const speakerNames = doc?.speakers?.names ?? NO_SPEAKERS;
	const speakerBefore = useMemo(() => speakersBefore(segments), [segments]);
	// An iPhone session is transcribed as a whole: every segment goes, and its
	// minutes are the whole session's.
	const wholeSession =
		Boolean(user?.transcription?.wholeSession) &&
		transcriptKey.startsWith("s:");
	const missing = untranscribed(segments);
	const toSend = wholeSession && missing.length > 0 ? segments : missing;
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
	const allSegments = useRef(segments);
	allSegments.current = segments;
	const renamedCount = useRef(0);
	renamedCount.current = renamedSpeakers(speakerNames);
	/**
	 * The whole session goes again: asks first when that drops edited
	 * sentences or speaker names.
	 */
	const transcribeWhole = useCallback(() => {
		const all = allSegments.current;
		const edited = all.reduce((sum, s) => sum + s.editedSentences, 0);
		const renamed = renamedCount.current;
		if (edited > 0 || renamed > 0) setRerunAll({ edited, renamed });
		else
			void startTranscribe.current(
				all.map((s) => s.fileId),
				all.reduce((sum, s) => sum + s.durationS, 0),
			);
	}, []);
	const requestRerun = useCallback(
		(segment: TranscriptSegment) => {
			if (wholeSession) {
				transcribeWhole();
				return;
			}
			if (segment.edited) setRerun(segment);
			else void startTranscribe.current([segment.fileId], segment.durationS);
		},
		[wholeSession, transcribeWhole],
	);

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
						{Object.keys(speakerNames).length > 0 ? (
							<IconButton
								label={t("transcript.speakers.open")}
								data-id="reader-speakers"
								icon={<IconUsers size={20} aria-hidden="true" />}
								onClick={() => setSpeakersOpen(true)}
							/>
						) : null}
						{titleDraft === null ? (
							<IconButton
								label={t("transcript.reader.rename")}
								data-id="reader-rename"
								icon={<IconPencil size={20} aria-hidden="true" />}
								onClick={() => setTitleDraft(doc.title ?? "")}
							/>
						) : null}
						{anyDone ? (
							<TranscriptDownloadMenu
								scope={scope}
								name={title}
								languages={available.map((a) => a.lang)}
								data-id="reader-download"
								compact
							/>
						) : null}
						<RowActionsMenu
							label={t("transcript.reader.menu")}
							data-id="reader-menu"
						>
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
							maxLength={MAX_TITLE_CHARS}
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
				{anyDone && (canTranscribe || available.length > 0) ? (
					<TranslationControls
						transcriptKey={transcriptKey}
						available={available}
						lang={lang}
						onLangChange={setChosenLang}
						view={shownView}
						onViewChange={setView}
						translation={translation}
						progress={progress}
						canTranslate={canTranscribe}
					/>
				) : null}
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
							onClick={() => {
								if (wholeSession && segments.some((s) => s.status === "done"))
									transcribeWhole();
								else
									void transcribe.start(
										toSend.map((s) => s.fileId),
										toSend.reduce((sum, s) => sum + s.durationS, 0),
									);
							}}
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
								view={shownView}
								translationLang={lang ?? undefined}
								translation={translated.get(segment.fileId)}
								onPlay={play}
								onEditingChange={onEditingChange}
								onSave={saveSegment}
								onRerun={requestRerun}
								speakerNames={speakerNames}
								speakerBefore={speakerBefore.get(segment.fileId) ?? null}
								rerunsSession={wholeSession}
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
			<SpeakerNamesDialog
				open={speakersOpen}
				onOpenChange={setSpeakersOpen}
				names={speakerNames}
				onSave={async (names) => {
					await renameSpeakers.mutateAsync({ key: transcriptKey, names });
					toast.show({ title: t("transcript.speakers.saved") });
				}}
			/>
			<ConfirmModal
				open={rerunAll !== null}
				onOpenChange={(next) => {
					if (!next) setRerunAll(null);
				}}
				title={t("transcript.reader.rerunAllTitle")}
				question={
					rerunAll && rerunAll.edited === 0
						? t("transcript.panel.rerunNamesQuestion", {
								count: rerunAll.renamed,
							})
						: t("transcript.reader.rerunAllQuestion", {
								count: rerunAll?.edited ?? 0,
							})
				}
				confirmLabel={t("transcript.panel.rerun")}
				onConfirm={() =>
					transcribe.start(
						segments.map((s) => s.fileId),
						totalS,
					)
				}
				data-id="reader-rerun-all"
			>
				{t("transcript.reader.rerunAllBody")}
				{rerunAll && rerunAll.renamed > 0
					? ` ${t("transcript.speakers.reset")}`
					: ""}
			</ConfirmModal>
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
