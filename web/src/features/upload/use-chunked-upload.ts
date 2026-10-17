import { useCallback, useRef, useState } from "react";
import { activeTranslator } from "../../i18n/active-translator";
import { errorMessage } from "../../lib/api";
import {
	ChunkedUpload,
	UPLOAD_CHUNK_SIZE,
	type UploadPurpose,
	type UploadResult,
} from "./chunked-upload";

export interface ChunkedUploadState {
	uploading: boolean;
	/** 0–100. */
	progress: number;
	error: string;
	result: UploadResult | null;
	/** Starts or resumes (same File after a failure) the upload. */
	upload: (file: File) => Promise<UploadResult | null>;
	reset: () => void;
}

/** React state around one `ChunkedUpload`; never throws. */
export function useChunkedUpload(purpose?: UploadPurpose): ChunkedUploadState {
	const uploader = useRef<ChunkedUpload | null>(null);
	const [uploading, setUploading] = useState(false);
	const [progress, setProgress] = useState(0);
	const [error, setError] = useState("");
	const [result, setResult] = useState<UploadResult | null>(null);

	const upload = useCallback(
		async (file: File) => {
			uploader.current ??= new ChunkedUpload(fetch, UPLOAD_CHUNK_SIZE, purpose);
			setUploading(true);
			setError("");
			setResult(null);
			try {
				const done = await uploader.current.upload(file, setProgress);
				setResult(done);
				return done;
			} catch (cause) {
				setError(
					errorMessage(cause, activeTranslator().t("errors.upload.failed")),
				);
				return null;
			} finally {
				setUploading(false);
			}
		},
		[purpose],
	);

	const reset = useCallback(() => {
		uploader.current?.reset();
		setUploading(false);
		setProgress(0);
		setError("");
		setResult(null);
	}, []);

	return { uploading, progress, error, result, upload, reset };
}
