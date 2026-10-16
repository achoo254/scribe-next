import { useQueryClient } from "@tanstack/react-query";
import {
	type ReactNode,
	createContext,
	useContext,
	useEffect,
	useState,
} from "react";
import { refreshStorage } from "../../features/files/file-queries";
import type { JobCriterion } from "../../features/split/split-job";
import {
	type SplitJobApi,
	useSplitJob,
} from "../../features/split/use-split-job";
import {
	type ChunkedUploadState,
	useChunkedUpload,
} from "../../features/upload/use-chunked-upload";

/*
 * "Cắt file" keeps its file here, above the routes: an upload of up to
 * 500 MB goes on while the user is on another page, and coming back shows
 * its progress and result. "Tải tiếp" needs the very same `File`, so it is
 * kept too. This job is its own instance, apart from the cut after a
 * recording (`RecordStateProvider`).
 */
export interface SplitPageState {
	upload: ChunkedUploadState;
	job: SplitJobApi;
	/** The chosen file, needed again to resume a failed upload. */
	file: File | null;
	criterion: JobCriterion;
	setCriterion: (criterion: JobCriterion) => void;
	value: string;
	setValue: (value: string) => void;
	/** Takes a new file: forgets the previous result, uploads this one. */
	choose: (file: File) => Promise<void>;
}

const SplitPageContext = createContext<SplitPageState | null>(null);

export function SplitPageProvider({ children }: { children: ReactNode }) {
	const queryClient = useQueryClient();
	const upload = useChunkedUpload();
	const job = useSplitJob();
	const [file, setFile] = useState<File | null>(null);
	const [criterion, setCriterion] = useState<JobCriterion>("minutes");
	const [value, setValue] = useState("10");

	// Uploads and segments count against the quota.
	useEffect(() => {
		if (upload.result) void refreshStorage(queryClient, ["managed"]);
	}, [upload.result, queryClient]);
	useEffect(() => {
		if (job.status === "done") void refreshStorage(queryClient, ["managed"]);
	}, [job.status, queryClient]);

	async function choose(next: File): Promise<void> {
		setFile(next);
		// Clear the previous file's result so its zip is not downloaded by mistake.
		job.reset();
		upload.reset();
		await upload.upload(next);
	}

	return (
		<SplitPageContext.Provider
			value={{
				upload,
				job,
				file,
				criterion,
				setCriterion,
				value,
				setValue,
				choose,
			}}
		>
			{children}
		</SplitPageContext.Provider>
	);
}

/** The "Cắt file" state, or null outside its provider (tests). */
export function useOptionalSplitPageState(): SplitPageState | null {
	return useContext(SplitPageContext);
}

export function useSplitPageState(): SplitPageState {
	const state = useContext(SplitPageContext);
	if (!state) throw new Error("useSplitPageState needs <SplitPageProvider>");
	return state;
}
