import { IconArrowLeft, IconPencil, IconTrash } from "@tabler/icons-react";
import { useQueryClient } from "@tanstack/react-query";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { AudioPlayer, type MediaSeek } from "../../components/media-player";
import { Alert } from "../../components/ui/alert";
import { Button } from "../../components/ui/button";
import { Card } from "../../components/ui/card";
import { ConfirmModal } from "../../components/ui/confirm-modal";
import { IconButton } from "../../components/ui/icon-button";
import { Input } from "../../components/ui/input";
import { Skeleton, SkeletonRegion } from "../../components/ui/skeleton";
import {
	activeSentence,
	gapsBefore,
	isPending,
	nextSegment,
	overallProgress,
	shouldPreload,
	transcriptTitle,
	untranscribed,
} from "../../features/transcript-docs/reader-model";
import type {
	TranscriptDetail,
	TranscriptSegment,
} from "../../features/transcript-docs/transcript-docs-api";
import {
	transcriptDocKeys,
	useDeleteTranscript,
	useRenameTranscript,
	useSaveSegment,
	useTranscriptDoc,
} from "../../features/transcript-docs/transcript-docs-queries";
import type { ExportScope } from "../../features/transcript/transcript-api";
import { LeaveGuard } from "../../layout/leave-guard";
import { errorMessage, isNativeShell } from "../../lib/api";
import {
	downloadPaths,
	preloadMedia,
	useDownload,
	useMediaSource,
} from "../../lib/download";
import { formatDateTime, formatDuration } from "../../lib/format";
import { useTranscribe } from "../record/use-transcribe";
import { ReaderSegment } from "./reader-segment";

export interface TranscriptReaderProps {
	transcriptKey: string;
	canTranscribe: boolean;
	/** `replace` after a delete, so back/swipe does not reopen the deleted doc. */
	onBack: (options?: { replace?: boolean }) => void;
}

interface Playback {
	fileId: string;
	seek: MediaSeek | null;
	autoPlay: boolean;
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
	canTranscribe,
	onBack,
}: TranscriptReaderProps) {
	const queryClient = useQueryClient();
	const download = useDownload();
	const { detail, statuses } = useTranscriptDoc(transcriptKey);
	const rename = useRenameTranscript();
	const save = useSaveSegment();
	const remove = useDeleteTranscript();
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
	const toSend = untranscribed(segments);
	const pending = segments.filter((s) => isPending(s.status)).length;

	const play = useCallback((fileId: string, seconds: number) => {
		setPlayback({ fileId, seek: { time: seconds }, autoPlay: false });
		setTime(seconds);
	}, []);

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
		} catch (cause) {
			setTitleError(errorMessage(cause));
		}
	}

	if (detail.isPending) {
		return (
			<Card className="grid gap-3 p-4 sm:p-6">
				<SkeletonRegion label="Đang tải transcript…" className="grid gap-3">
					<Skeleton className="h-8" />
					<Skeleton className="h-40" />
				</SkeletonRegion>
			</Card>
		);
	}
	if (detail.isError || !doc) {
		return (
			<Card className="grid gap-3 p-4 sm:p-6">
				<BackButton onBack={onBack} />
				<Alert tone="danger" data-id="reader-error">
					{detail.error?.message ?? "Không tải được transcript"}
				</Alert>
			</Card>
		);
	}

	const title = transcriptTitle(doc);
	const totalS = segments.reduce((sum, s) => sum + s.durationS, 0);
	const anyDone = segments.some((s) => s.status === "done");
	const scope = exportScope(doc);

	return (
		<Card data-id="reader" className="grid gap-4 p-4 sm:p-6">
			<div className="grid gap-2">
				<BackButton onBack={onBack} />
				{titleDraft === null ? (
					<div className="flex items-start gap-2">
						<h2 className="m-0 min-w-0 flex-1 break-words font-bold font-mono text-lg">
							{title}
						</h2>
						<IconButton
							size="sm"
							variant="ghost"
							label="Đổi tên"
							data-id="reader-rename"
							icon={<IconPencil size={18} aria-hidden="true" />}
							onClick={() => setTitleDraft(doc.title ?? "")}
						/>
					</div>
				) : (
					<form
						className="grid gap-2"
						onSubmit={(event) => {
							event.preventDefault();
							void submitTitle();
						}}
					>
						<Input
							data-id="reader-title-input"
							aria-label="Tên transcript"
							placeholder={transcriptTitle({ ...doc, title: null })}
							value={titleDraft}
							maxLength={120}
							autoFocus
							onChange={(event) => setTitleDraft(event.target.value)}
						/>
						{titleError ? <Alert tone="danger">{titleError}</Alert> : null}
						<div className="flex flex-wrap justify-end gap-2">
							<Button
								variant="ghost"
								data-id="reader-title-cancel"
								disabled={rename.isPending}
								onClick={() => {
									setTitleDraft(null);
									setTitleError("");
								}}
							>
								Huỷ
							</Button>
							<Button
								type="submit"
								variant="primary"
								data-id="reader-title-save"
								loading={rename.isPending}
							>
								Lưu tên
							</Button>
						</div>
						<p className="m-0 text-muted text-sm">
							Để trống để dùng tên mặc định.
						</p>
					</form>
				)}
				<div className="text-muted text-sm">
					{formatDateTime(doc.createdAt ?? undefined)} ·{" "}
					{formatDuration(totalS)}
					{segments.length > 1 ? ` · ${segments.length} đoạn` : ""}
				</div>
				<div className="flex flex-wrap items-center gap-1">
					{anyDone ? (
						<>
							<Button
								size="sm"
								variant="ghost"
								data-id="reader-txt"
								onClick={() =>
									download(downloadPaths.transcript(scope, "txt", title))
								}
							>
								{isNativeShell() ? "Lưu .txt" : "Tải .txt"}
							</Button>
							<Button
								size="sm"
								variant="ghost"
								data-id="reader-srt"
								onClick={() =>
									download(downloadPaths.transcript(scope, "srt", title))
								}
							>
								{isNativeShell() ? "Lưu .srt" : "Tải .srt"}
							</Button>
						</>
					) : null}
					<Button
						size="sm"
						variant="ghost"
						data-id="reader-delete"
						className="text-error!"
						leadingIcon={<IconTrash size={16} aria-hidden="true" />}
						onClick={() => setConfirmDelete(true)}
					>
						Xoá transcript
					</Button>
				</div>
			</div>

			{pending > 0 ? (
				<Alert tone="info" data-id="reader-progress">
					Đang chép lời… {overallProgress(segments, statuses)}%
				</Alert>
			) : null}
			{canTranscribe && pending === 0 && toSend.length > 0 ? (
				<Alert tone="warning" data-id="reader-unsent">
					Còn {toSend.length} đoạn chưa chép.
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
						Chép {toSend.length} đoạn còn lại
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
								Thiếu đoạn {missing + 1}
							</p>
						))}
						<ReaderSegment
							segment={segment}
							numbered={transcriptKey.startsWith("s:")}
							progress={live.get(segment.fileId)?.progress ?? segment.progress}
							activeIndex={
								segment.fileId === current?.fileId && playback
									? activeSentence(segment, time)
									: -1
							}
							editLocked={editing !== null && editing !== segment.fileId}
							canTranscribe={canTranscribe}
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
							Đang ở đoạn {current.index + 1}
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

			{transcribe.dialog}
			<LeaveGuard when={unsaved} data-id="reader-leave" />
			<ConfirmModal
				open={rerun !== null}
				onOpenChange={(next) => {
					if (!next) setRerun(null);
				}}
				title="Chép lại đoạn"
				question={`Bỏ ${rerun?.editedSentences ?? 0} câu đã sửa ở đoạn ${(rerun?.index ?? 0) + 1}?`}
				confirmLabel="Chép lại"
				onConfirm={() =>
					transcribe.start(rerun ? [rerun.fileId] : [], rerun?.durationS ?? 0)
				}
				data-id="reader-rerun"
			>
				Đoạn này sẽ về câu máy mới; các đoạn khác giữ phần đã sửa.
			</ConfirmModal>
			<ConfirmModal
				open={confirmDelete}
				onOpenChange={setConfirmDelete}
				title="Xoá transcript"
				question={`Xoá transcript “${title}”?`}
				confirmLabel="Xoá"
				onConfirm={async () => {
					await remove.mutateAsync(transcriptKey);
					setRemoved(true);
				}}
				data-id="reader-delete-confirm"
			>
				Phần chữ và phần đã sửa sẽ mất. Audio vẫn còn trong Lịch sử ghi.
			</ConfirmModal>
		</Card>
	);
}

function BackButton({ onBack }: { onBack: () => void }) {
	return (
		<Button
			size="sm"
			variant="ghost"
			className="justify-self-start"
			data-id="reader-back"
			leadingIcon={<IconArrowLeft size={16} aria-hidden="true" />}
			onClick={() => onBack()}
		>
			Danh sách
		</Button>
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
	const { src, error } = useMediaSource(segment.fileId);
	if (error) {
		return (
			<p role="alert" className="m-0 text-error text-sm">
				Không phát được: {error}
			</p>
		);
	}
	if (!src) {
		return (
			<p aria-live="polite" className="m-0 text-muted text-sm">
				Đang tải audio…
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
