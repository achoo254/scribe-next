import {
	IconDownload,
	IconFolderOpen,
	IconRefresh,
	IconScissors,
} from "@tabler/icons-react";
import { type ReactNode, useRef } from "react";
import { Alert } from "../../components/ui/alert";
import { Button } from "../../components/ui/button";
import { Card } from "../../components/ui/card";
import { MeterBar } from "../../components/ui/meter-bar";
import { ProgressBar } from "../../components/ui/progress-bar";
import { isJobBusy } from "../../features/split/split-job";
import { useT } from "../../i18n/i18n-context";
import { PageHeader } from "../../layout/page-header";
import { downloadPaths, useDownload } from "../../lib/download";
import { formatDuration } from "../../lib/format";
import { ManagedFilesCard } from "./managed-files-card";
import { SplitCriteria, parseSplitValue } from "./split-criteria";
import { useSplitPageState } from "./split-state";

/**
 * "Cắt file" page (Mode B): upload a file → choose how to cut → download
 * the segments as a zip, one numbered step after the other. File, upload
 * and job live in `SplitPageProvider`, so they go on while another page
 * is shown.
 */
export function SplitPage() {
	const t = useT();
	const download = useDownload();
	const {
		upload,
		job,
		file,
		criterion,
		setCriterion,
		value,
		setValue,
		choose,
	} = useSplitPageState();
	const input = useRef<HTMLInputElement>(null);
	const amount = parseSplitValue(criterion, value);
	const busy = isJobBusy(job.status);

	return (
		<section data-id="split" className="grid gap-5">
			<PageHeader
				title={t("split.title")}
				description={t("split.description")}
			/>
			<Card data-id="split-card" className="grid gap-6 p-4 sm:p-6">
				<SplitStep number={1} title={t("split.steps.file")}>
					<div className="flex flex-wrap items-center gap-3">
						<input
							ref={input}
							type="file"
							accept="audio/*,video/*"
							className="hidden"
							data-id="split-file-input"
							onChange={(event) => {
								const next = event.target.files?.[0];
								if (next) void choose(next);
								// The same file can be chosen again later.
								event.target.value = "";
							}}
						/>
						<Button
							variant={file ? "secondary" : "primary"}
							data-id="split-choose"
							disabled={upload.uploading}
							leadingIcon={<IconFolderOpen size={18} aria-hidden="true" />}
							onClick={() => input.current?.click()}
						>
							{file ? t("split.chooseAnother") : t("split.choose")}
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
								label={t("split.uploadProgress")}
								value={upload.progress}
								data-id="split-upload-progress"
							/>
							<p className="m-0 text-muted text-sm" aria-live="polite">
								{t("split.uploading", { percent: upload.progress })}
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
									{t("split.resume")}
								</Button>
							</div>
						</Alert>
					) : null}
					{upload.result ? (
						<Alert tone="success" data-id="split-uploaded">
							{t("split.uploaded", {
								duration: formatDuration(upload.result.durationS),
							})}
						</Alert>
					) : null}
				</SplitStep>

				{upload.result ? (
					<SplitStep number={2} title={t("split.steps.criteria")}>
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
							{t("split.start")}
						</Button>
						{busy ? (
							<div className="grid gap-2" aria-busy="true">
								<ProgressBar className="relative rounded-full" />
								<p className="m-0 text-muted text-sm">{t("split.working")}</p>
							</div>
						) : null}
						{job.error ? <Alert tone="danger">{job.error}</Alert> : null}
					</SplitStep>
				) : null}

				{job.status === "done" && job.jobId ? (
					<SplitStep
						number={3}
						title={t("split.result", { count: job.segments.length })}
						data-id="split-result"
					>
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
							{t("split.downloadZip")}
						</Button>
					</SplitStep>
				) : null}
			</Card>

			<ManagedFilesCard />
		</section>
	);
}

interface SplitStepProps {
	number: number;
	title: string;
	children: ReactNode;
	"data-id"?: string;
}

/** One step of the card: a numbered heading, then its controls. */
function SplitStep({
	number,
	title,
	children,
	"data-id": dataId,
}: SplitStepProps) {
	return (
		<section
			data-id={dataId}
			className="grid gap-3 border-border-subtle border-t pt-5 first:border-t-0 first:pt-0"
		>
			<h2 className="m-0 flex items-center gap-2 font-mono font-semibold text-base">
				<span
					aria-hidden="true"
					className="grid size-6 place-items-center rounded-full bg-primary/10 text-primary text-xs"
				>
					{number}
				</span>
				{title}
			</h2>
			{children}
		</section>
	);
}
