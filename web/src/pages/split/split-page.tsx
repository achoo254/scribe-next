import {
	IconDownload,
	IconFolderOpen,
	IconRefresh,
	IconScissors,
} from "@tabler/icons-react";
import { useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";
import { Alert } from "../../components/ui/alert";
import { Button } from "../../components/ui/button";
import { Card } from "../../components/ui/card";
import { MeterBar } from "../../components/ui/meter-bar";
import { ProgressBar } from "../../components/ui/progress-bar";
import { refreshStorage } from "../../features/files/file-queries";
import { type JobCriterion, isJobBusy } from "../../features/split/split-job";
import { useSplitJob } from "../../features/split/use-split-job";
import { useChunkedUpload } from "../../features/upload/use-chunked-upload";
import { downloadPaths, useDownload } from "../../lib/download";
import { formatDuration } from "../../lib/format";
import { ManagedFilesCard } from "./managed-files-card";
import { SplitCriteria, parseSplitValue } from "./split-criteria";

/**
 * "Cắt file" tab (Mode B): upload a file → choose how to cut → download
 * the segments as a zip. `active` is true while the tab is shown.
 */
export function SplitPage({ active }: { active: boolean }) {
	const queryClient = useQueryClient();
	const download = useDownload();
	const upload = useChunkedUpload();
	const job = useSplitJob();
	const input = useRef<HTMLInputElement>(null);
	const [file, setFile] = useState<File | null>(null);
	const [criterion, setCriterion] = useState<JobCriterion>("minutes");
	const [value, setValue] = useState("10");
	const amount = parseSplitValue(criterion, value);
	const busy = isJobBusy(job.status);

	// Uploads and segments count against the quota.
	useEffect(() => {
		if (upload.result) void refreshStorage(queryClient, ["managed"]);
	}, [upload.result, queryClient]);
	useEffect(() => {
		if (job.status === "done") void refreshStorage(queryClient, ["managed"]);
	}, [job.status, queryClient]);

	async function choose(next: File | undefined): Promise<void> {
		if (!next) return;
		setFile(next);
		// Clear the previous file's result so its zip is not downloaded by mistake.
		job.reset();
		upload.reset();
		await upload.upload(next);
	}

	return (
		<div data-id="split" className="grid gap-4">
			<Card className="grid gap-4 p-4 sm:p-6">
				<h2 className="m-0 font-bold font-mono text-lg">Cắt file</h2>
				<div className="flex flex-wrap items-center gap-3">
					<input
						ref={input}
						type="file"
						accept="audio/*,video/*"
						className="hidden"
						data-id="split-file-input"
						onChange={(event) => {
							void choose(event.target.files?.[0]);
							// The same file can be chosen again later.
							event.target.value = "";
						}}
					/>
					<Button
						data-id="split-choose"
						disabled={upload.uploading}
						leadingIcon={<IconFolderOpen size={18} aria-hidden="true" />}
						onClick={() => input.current?.click()}
					>
						Chọn file audio/video
					</Button>
					{file ? (
						<span
							className="min-w-0 truncate text-muted text-sm"
							title={file.name}
						>
							{file.name}
						</span>
					) : null}
				</div>

				{upload.uploading ? (
					<div className="grid gap-2">
						<MeterBar
							label="Tiến độ tải lên"
							value={upload.progress}
							data-id="split-upload-progress"
						/>
						<p className="m-0 text-muted text-sm" aria-live="polite">
							Đang tải lên… {upload.progress}%
						</p>
					</div>
				) : null}
				{upload.error && file ? (
					<Alert tone="danger" data-id="split-upload-error">
						<div className="grid justify-items-start gap-2">
							<span>{upload.error}</span>
							<Button
								size="sm"
								data-id="split-upload-retry"
								leadingIcon={<IconRefresh size={16} aria-hidden="true" />}
								onClick={() => void upload.upload(file)}
							>
								Tải tiếp
							</Button>
						</div>
					</Alert>
				) : null}
				{upload.result ? (
					<Alert tone="success" data-id="split-uploaded">
						Đã tải lên — thời lượng {formatDuration(upload.result.durationS)}
					</Alert>
				) : null}

				{upload.result ? (
					<div className="grid gap-4">
						<SplitCriteria
							criterion={criterion}
							onCriterionChange={setCriterion}
							value={value}
							onValueChange={setValue}
							disabled={busy}
						/>
						<Button
							variant="primary"
							size="lg"
							className="justify-self-start"
							data-id="split-start"
							loading={job.creating || busy}
							disabled={upload.uploading || amount === null}
							leadingIcon={<IconScissors size={20} aria-hidden="true" />}
							onClick={() => {
								if (upload.result && amount !== null) {
									void job.start({
										inputFileId: upload.result.fileId,
										criterion,
										value: amount,
									});
								}
							}}
						>
							Cắt
						</Button>
						{busy ? (
							<div className="grid gap-2" aria-busy="true">
								<ProgressBar className="relative rounded-full" />
								<p className="m-0 text-muted text-sm">Đang xử lý…</p>
							</div>
						) : null}
						{job.error ? <Alert tone="danger">{job.error}</Alert> : null}
					</div>
				) : null}

				{job.status === "done" && job.jobId ? (
					<section data-id="split-result" className="grid gap-3">
						<h3 className="m-0 font-mono font-semibold text-base">
							Kết quả ({job.segments.length} đoạn)
						</h3>
						<ul className="m-0 grid max-h-55 list-disc gap-1 overflow-y-auto pl-5 text-muted text-sm">
							{job.segments.map((segment) => (
								<li key={segment.fileId}>{segment.name}</li>
							))}
						</ul>
						<Button
							variant="primary"
							className="justify-self-start"
							data-id="split-download"
							leadingIcon={<IconDownload size={18} aria-hidden="true" />}
							onClick={() =>
								job.jobId && download(downloadPaths.jobResult(job.jobId))
							}
						>
							Tải zip
						</Button>
					</section>
				) : null}
			</Card>

			<ManagedFilesCard active={active} />
		</div>
	);
}
