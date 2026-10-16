import { useCallback, useEffect, useState } from "react";
import { useToast } from "../components/ui/toast";
import { transcriptExportPath } from "../features/transcript/transcript-api";
import { apiUrl, errorMessage, isNativeShell } from "./api";
import { NativeFiles } from "./native";

/**
 * Downloads an `/api/...` URL. On the web the backend answers with
 * `Content-Disposition: attachment`, so navigating keeps the page and the
 * browser saves the file. In the iOS app the WebView would leave the app
 * without the session cookie, so the native plugin downloads it and opens
 * the share sheet (Save to Files, AirDrop…).
 */
export async function download(path: string): Promise<void> {
	if (isNativeShell()) {
		await NativeFiles.share({ url: apiUrl(path) });
		return;
	}
	window.location.href = apiUrl(path);
}

/** `download` that reports failures as a toast; for buttons. */
export function useDownload(): (path: string) => void {
	const toast = useToast();
	return useCallback(
		(path: string) => {
			download(path).catch((cause: unknown) =>
				toast.show({
					tone: "error",
					title: "Không tải được tệp",
					description: errorMessage(cause),
				}),
			);
		},
		[toast],
	);
}

/** Stream URL of a stored file (HTTP Range, so players can seek). */
export function mediaUrl(fileId: string): string {
	return apiUrl(`/api/files/${encodeURIComponent(fileId)}`);
}

export interface MediaSource {
	/** Playable URL; null while the app is still fetching the file. */
	src: string | null;
	error: string;
}

/**
 * Source for `<audio>`/`<video>`. The web streams with Range requests; the
 * iOS app first copies the file into its cache (a media element cannot send
 * the session cookie cross-origin), then plays the local copy.
 */
export function useMediaSource(fileId: string): MediaSource {
	const native = isNativeShell();
	// The app's answer is kept with the file it belongs to: right after
	// `fileId` changes, the render must not hand out the previous file.
	const [loaded, setLoaded] = useState<MediaSource & { fileId: string }>({
		fileId: "",
		src: null,
		error: "",
	});

	useEffect(() => {
		if (!native) return;
		let cancelled = false;
		NativeFiles.cache({ url: mediaUrl(fileId) })
			.then(({ url }) => {
				if (!cancelled) setLoaded({ fileId, src: url, error: "" });
			})
			.catch((cause: unknown) => {
				if (!cancelled)
					setLoaded({ fileId, src: null, error: errorMessage(cause) });
			});
		return () => {
			cancelled = true;
		};
	}, [fileId, native]);

	if (!native) return { src: mediaUrl(fileId), error: "" };
	return loaded.fileId === fileId
		? { src: loaded.src, error: loaded.error }
		: { src: null, error: "" };
}

/**
 * Starts loading a file that will play soon (the next segment). The web
 * streams on demand, so only the iOS app copies it into its cache ahead.
 */
export function preloadMedia(fileId: string): void {
	if (!isNativeShell()) return;
	NativeFiles.cache({ url: mediaUrl(fileId) }).catch(() => undefined);
}

/** Paths of the download endpoints, in one place. */
export const downloadPaths = {
	file: (fileId: string) =>
		`/api/files/${encodeURIComponent(fileId)}?download=1`,
	allRecordings: () => "/api/files/archive",
	recordingsBetween: (from: number, to: number) =>
		`/api/files/archive?from=${from}&to=${to}`,
	jobResult: (jobId: string) =>
		`/api/jobs/${encodeURIComponent(jobId)}/download`,
	transcript: transcriptExportPath,
} as const;
