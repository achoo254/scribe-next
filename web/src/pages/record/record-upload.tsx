import { IconRefresh, IconUpload } from "@tabler/icons-react";
import { useRef } from "react";
import { Alert } from "../../components/ui/alert";
import { Button } from "../../components/ui/button";
import { MeterBar } from "../../components/ui/meter-bar";
import { useT } from "../../i18n/i18n-context";
import { renderRich } from "../../i18n/rich-text";
import { useRecordUpload } from "./record-upload-state";

/**
 * Anything ffmpeg can decode works for transcription; the backend accepts
 * audio and video it recognises by content.
 */
export const UPLOAD_ACCEPT = "audio/*,video/*";

/**
 * "Tải tệp lên": an audio or video file from this device goes to "Lịch sử
 * ghi" like a recording, where it can be played and transcribed. Uses the
 * same chunked upload as "Cắt file"; a failed upload resumes.
 */
export function RecordUpload() {
	const t = useT();
	const { upload, file, choose } = useRecordUpload();
	const input = useRef<HTMLInputElement>(null);

	return (
		<div data-id="record-upload" className="grid gap-2">
			<div className="flex flex-wrap items-center gap-3">
				<input
					ref={input}
					type="file"
					accept={UPLOAD_ACCEPT}
					className="hidden"
					data-id="record-upload-input"
					onChange={(event) => {
						const next = event.target.files?.[0];
						if (next) void choose(next);
						// The same file can be chosen again later.
						event.target.value = "";
					}}
				/>
				<Button
					variant="secondary"
					data-id="record-upload-choose"
					loading={upload.uploading}
					leadingIcon={<IconUpload size={18} aria-hidden="true" />}
					onClick={() => input.current?.click()}
				>
					{t("record.upload.choose")}
				</Button>
				<span className="text-muted text-sm">{t("record.upload.hint")}</span>
			</div>
			{upload.uploading ? (
				<div className="grid gap-1">
					<MeterBar
						label={t("record.upload.progress")}
						value={upload.progress}
						data-id="record-upload-progress"
					/>
					<p className="m-0 text-muted text-sm" aria-live="polite">
						{t("record.upload.uploading", {
							name: file?.name ?? "",
							percent: upload.progress,
						})}
					</p>
				</div>
			) : null}
			{upload.error && file ? (
				<Alert tone="danger" data-id="record-upload-error">
					<div className="grid justify-items-start gap-2">
						<span>{upload.error}</span>
						<Button
							size="sm"
							data-id="record-upload-retry"
							leadingIcon={<IconRefresh size={16} aria-hidden="true" />}
							onClick={() => void upload.upload(file)}
						>
							{t("record.upload.resume")}
						</Button>
					</div>
				</Alert>
			) : null}
			{upload.result ? (
				<Alert tone="success" data-id="record-upload-done">
					{renderRich(t("record.upload.done", { name: upload.result.name }))}
				</Alert>
			) : null}
		</div>
	);
}
