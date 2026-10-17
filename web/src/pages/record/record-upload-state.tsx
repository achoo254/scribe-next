import { useQueryClient } from "@tanstack/react-query";
import {
	type ReactNode,
	createContext,
	useContext,
	useEffect,
	useState,
} from "react";
import { refreshStorage } from "../../features/files/file-queries";
import {
	type ChunkedUploadState,
	useChunkedUpload,
} from "../../features/upload/use-chunked-upload";

/*
 * "Tải tệp lên" on the record page keeps its upload here, above the routes
 * like the recorder: leaving /record does not stop it, and coming back
 * shows its progress. Its own instance, apart from the "Cắt file" upload.
 */
export interface RecordUploadState {
	upload: ChunkedUploadState;
	/** The chosen file, needed again to resume a failed upload. */
	file: File | null;
	/** Takes a new file and uploads it. */
	choose: (file: File) => Promise<void>;
}

const RecordUploadContext = createContext<RecordUploadState | null>(null);

export function RecordUploadProvider({ children }: { children: ReactNode }) {
	const queryClient = useQueryClient();
	const upload = useChunkedUpload();
	const [file, setFile] = useState<File | null>(null);

	// The uploaded file joins "Lịch sử ghi" and counts against the quota.
	useEffect(() => {
		if (upload.result) void refreshStorage(queryClient, ["recordings"]);
	}, [upload.result, queryClient]);

	async function choose(next: File): Promise<void> {
		setFile(next);
		upload.reset();
		await upload.upload(next);
	}

	return (
		<RecordUploadContext.Provider value={{ upload, file, choose }}>
			{children}
		</RecordUploadContext.Provider>
	);
}

/** The record-page upload, or null outside its provider (tests). */
export function useOptionalRecordUpload(): RecordUploadState | null {
	return useContext(RecordUploadContext);
}

export function useRecordUpload(): RecordUploadState {
	const state = useContext(RecordUploadContext);
	if (!state) throw new Error("useRecordUpload needs <RecordUploadProvider>");
	return state;
}
