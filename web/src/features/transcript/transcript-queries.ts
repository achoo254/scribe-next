import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { fileKeys } from "../files/file-queries";
import {
	type Transcript,
	fetchTranscripts,
	requestTranscripts,
} from "./transcript-api";

const POLL_MS = 2000;

export const transcriptKeys = {
	all: ["transcripts"] as const,
	of: (fileIds: readonly string[]) =>
		["transcripts", fileIds.join(",")] as const,
};

function hasPending(items: readonly Transcript[] | undefined): boolean {
	return (items ?? []).some(
		(t) => t.status === "queued" || t.status === "running",
	);
}

/**
 * Transcripts of the files of one recording; polls while any is still
 * queued or running. `enabled` stays off until something was requested,
 * so a long history does not fetch every row.
 */
export function useTranscriptsQuery(
	fileIds: readonly string[],
	enabled: boolean,
) {
	return useQuery({
		queryKey: transcriptKeys.of(fileIds),
		queryFn: () => fetchTranscripts(fileIds),
		enabled: enabled && fileIds.length > 0,
		refetchInterval: (query) =>
			hasPending(query.state.data) ? POLL_MS : false,
	});
}

export function useRequestTranscripts() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: ({
			fileIds,
			apiKey,
		}: {
			fileIds: readonly string[];
			apiKey: string;
		}) => requestTranscripts(fileIds, apiKey),
		onSuccess: () =>
			Promise.all([
				queryClient.invalidateQueries({ queryKey: transcriptKeys.all }),
				queryClient.invalidateQueries({ queryKey: fileKeys.recordings }),
			]),
	});
}
