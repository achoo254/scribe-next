import { IconArrowRight, IconFileText } from "@tabler/icons-react";
import { useQueryClient } from "@tanstack/react-query";
import { useMemo, useState } from "react";
import { Alert } from "../../components/ui/alert";
import { Button } from "../../components/ui/button";
import { ConfirmModal } from "../../components/ui/confirm-modal";
import { useToast } from "../../components/ui/toast";
import { editedCounts } from "../../features/transcript-docs/reader-model";
import {
	fetchTranscriptDoc,
	transcriptKey,
} from "../../features/transcript-docs/transcript-docs-api";
import { transcriptDocKeys } from "../../features/transcript-docs/transcript-docs-queries";
import { forgetApiKey } from "../../features/transcript/api-key";
import {
	type TranscriptPart,
	mergeTranscripts,
} from "../../features/transcript/merge-transcripts";
import type { ExportScope } from "../../features/transcript/transcript-api";
import { useTranscriptsQuery } from "../../features/transcript/transcript-queries";
import { useOpenTranscript } from "../../layout/transcript-nav";
import { errorMessage, isNativeShell } from "../../lib/api";
import { downloadPaths, useDownload } from "../../lib/download";
import { formatClock } from "../../lib/format";
import { useTranscribe } from "./use-transcribe";

/** OpenAI refused the key itself: asking for another one is the fix. */
const KEY_PROBLEM = /API key|credit/i;

export interface TranscriptPanelProps {
	/** Files of the recording in playback order (one, or an iPhone session's segments). */
	parts: readonly TranscriptPart[];
	/** Some file was already sent for transcription (from the recordings list). */
	requested: boolean;
	/** File name of the .txt/.srt download. */
	name: string;
	/** iPhone session id: the download then covers every segment, however many. */
	session?: string;
	onSeek: (fileId: string, seconds: number) => void;
	"data-id": string;
}

/**
 * "Chép lời" for one recording: asks for the user's OpenAI key the first
 * time, shows progress while the backend transcribes, then the sentences;
 * a click on one seeks the player there.
 */
export function TranscriptPanel({
	parts,
	requested,
	name,
	session,
	onSeek,
	"data-id": dataId,
}: TranscriptPanelProps) {
	const toast = useToast();
	const download = useDownload();
	const queryClient = useQueryClient();
	const openTranscript = useOpenTranscript();
	const fileIds = useMemo(() => parts.map((p) => p.fileId), [parts]);
	const [asked, setAsked] = useState(false);
	const [open, setOpen] = useState(false);
	const transcribe = useTranscribe(() => {
		setAsked(true);
		setOpen(true);
		void queryClient.invalidateQueries({ queryKey: transcriptDocKeys.all });
	});
	const query = useTranscriptsQuery(fileIds, requested || asked);
	const merged = useMemo(
		() => mergeTranscripts(parts, query.data ?? []),
		[parts, query.data],
	);
	/** Edits a full re-transcription would discard, waiting for confirmation. */
	const [lostEdits, setLostEdits] = useState<{
		segments: number;
		sentences: number;
	} | null>(null);
	const docKey = transcriptKey(
		session ? { session } : { fileId: fileIds[0] ?? "" },
	);

	const busy = merged.pending > 0 || transcribe.pending;
	const allDone = merged.total > 0 && merged.done === merged.total;
	/** Files to send now: never sent or failed; everything when all are done. */
	const toSend = allDone
		? fileIds
		: fileIds.filter((id) => {
				const t = query.data?.find((item) => item.fileId === id);
				return !t || t.status === "failed";
			});
	const keyProblem = merged.failed > 0 && KEY_PROBLEM.test(merged.error);
	const scope: ExportScope = session ? { session } : { fileIds };

	/** "Chép lại" over edited sentences asks first: those edits would be lost. */
	async function start(): Promise<void> {
		if (!allDone) {
			await transcribe.start(toSend);
			return;
		}
		try {
			const detail = await queryClient.fetchQuery({
				queryKey: transcriptDocKeys.detail(docKey),
				queryFn: () => fetchTranscriptDoc(docKey),
			});
			const edited = editedCounts(detail.segments);
			if (edited.segments > 0) {
				setLostEdits(edited);
				return;
			}
		} catch (cause) {
			toast.show({
				tone: "error",
				title: "Không kiểm được phần đã sửa",
				description: errorMessage(cause),
			});
			return;
		}
		await transcribe.start(toSend);
	}

	const status = merged.untouched
		? null
		: busy
			? `Đang chép lời… ${merged.progress}%`
			: allDone
				? "Đã chép lời"
				: merged.done > 0
					? `Đã chép ${merged.done}/${merged.total} đoạn`
					: "";

	return (
		<div data-id={dataId} className="grid gap-2">
			<div className="flex flex-wrap items-center gap-x-3 gap-y-1">
				{status ? (
					<span
						className="text-muted text-sm"
						aria-live="polite"
						data-id={`${dataId}-status`}
					>
						{status}
					</span>
				) : null}
				{merged.done > 0 && openTranscript ? (
					<Button
						size="sm"
						variant="ghost"
						data-id={`${dataId}-open`}
						leadingIcon={<IconArrowRight size={16} aria-hidden="true" />}
						onClick={() => openTranscript(docKey)}
					>
						Mở transcript
					</Button>
				) : null}
				{merged.lines.length > 0 ? (
					<Button
						size="sm"
						variant="ghost"
						data-id={`${dataId}-toggle`}
						aria-expanded={open}
						leadingIcon={<IconFileText size={16} aria-hidden="true" />}
						onClick={() => setOpen((value) => !value)}
					>
						{open ? "Ẩn chữ" : "Xem chữ"}
					</Button>
				) : null}
				{!busy && toSend.length > 0 ? (
					<Button
						size="sm"
						variant="ghost"
						data-id={`${dataId}-start`}
						leadingIcon={
							merged.untouched ? (
								<IconFileText size={16} aria-hidden="true" />
							) : undefined
						}
						onClick={() => void start()}
					>
						{merged.untouched
							? "Chép lời"
							: allDone
								? "Chép lại"
								: "Chép phần còn thiếu"}
					</Button>
				) : null}
				{merged.done > 0 && !busy ? (
					<>
						<Button
							size="sm"
							variant="ghost"
							data-id={`${dataId}-txt`}
							onClick={() =>
								download(downloadPaths.transcript(scope, "txt", name))
							}
						>
							{isNativeShell() ? "Lưu .txt" : "Tải .txt"}
						</Button>
						<Button
							size="sm"
							variant="ghost"
							data-id={`${dataId}-srt`}
							onClick={() =>
								download(downloadPaths.transcript(scope, "srt", name))
							}
						>
							{isNativeShell() ? "Lưu .srt" : "Tải .srt"}
						</Button>
					</>
				) : null}
			</div>

			{merged.failed > 0 && !busy ? (
				<Alert tone="danger" data-id={`${dataId}-error`}>
					{merged.error || "Chép lời thất bại."}
					{keyProblem ? (
						<Button
							size="sm"
							variant="ghost"
							className="ml-2"
							data-id={`${dataId}-change-key`}
							onClick={() => {
								void forgetApiKey();
								transcribe.askKey(toSend, merged.error);
							}}
						>
							Đổi key
						</Button>
					) : null}
				</Alert>
			) : null}

			{open && merged.lines.length > 0 ? (
				<ol
					data-id={`${dataId}-lines`}
					className="m-0 grid max-h-96 list-none gap-1 overflow-y-auto rounded-md border border-border-subtle bg-surface-subtle p-2"
				>
					{merged.lines.map((line, index) => (
						<li key={`${line.fileId}-${index}`}>
							<button
								type="button"
								data-id={`${dataId}-line-${index}`}
								onClick={() => onSeek(line.fileId, line.start)}
								className="flex w-full gap-3 rounded-sm px-2 py-1 text-left text-sm hover:bg-surface focus-visible:shadow-focus-primary focus-visible:outline-none"
							>
								<span className="flex-none font-mono text-muted tabular-nums">
									{formatClock(line.at)}
								</span>
								<span>{line.text}</span>
							</button>
						</li>
					))}
				</ol>
			) : null}

			{transcribe.keyModal}
			<ConfirmModal
				open={lostEdits !== null}
				onOpenChange={(next) => {
					if (!next) setLostEdits(null);
				}}
				title="Chép lại"
				question={`Bỏ phần đã sửa ở ${lostEdits?.segments ?? 0} đoạn (${lostEdits?.sentences ?? 0} câu)?`}
				confirmLabel="Chép lại"
				onConfirm={() => transcribe.start(toSend)}
				data-id={`${dataId}-rerun`}
			>
				Chép lại cả bản ghi thay mọi câu bằng bản chép mới. Muốn giữ phần đã
				sửa, hãy chép lại từng đoạn trong tab Transcript.
			</ConfirmModal>
		</div>
	);
}
