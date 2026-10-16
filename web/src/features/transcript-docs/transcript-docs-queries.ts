import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";
import { fileKeys } from "../files/file-queries";
import { transcriptKeys } from "../transcript/transcript-queries";
import { isPending, needsReload } from "./reader-model";
import {
	type TranscriptSummary,
	deleteTranscript,
	fetchTranscriptDoc,
	fetchTranscriptStatus,
	listTranscriptDocs,
	renameTranscript,
	saveSegmentTexts,
} from "./transcript-docs-api";

/** Status poll of an open transcript while segments are being transcribed. */
const STATUS_POLL_MS = 2000;
/** The list is lighter on detail; it only refreshes counts and previews. */
const LIST_POLL_MS = 5000;

export const transcriptDocKeys = {
	all: ["transcript-docs"] as const,
	list: ["transcript-docs", "list"] as const,
	detail: (key: string) => ["transcript-docs", "detail", key] as const,
	status: (key: string) => ["transcript-docs", "status", key] as const,
};

function anyPending(items: readonly TranscriptSummary[] | undefined): boolean {
	return (items ?? []).some((item) => item.pending > 0);
}

/** Transcripts of the user; polls while the page is open and something runs. */
export function useTranscriptDocsQuery() {
	return useQuery({
		queryKey: transcriptDocKeys.list,
		queryFn: () => listTranscriptDocs(),
		refetchOnMount: "always",
		refetchInterval: (query) =>
			anyPending(query.state.data) ? LIST_POLL_MS : false,
	});
}

/**
 * One transcript. While a segment is queued or running, only the light
 * `/status` route is polled; the full transcript reloads when a segment
 * changes state (finished, failed, queued again).
 */
export function useTranscriptDoc(key: string) {
	const queryClient = useQueryClient();
	const detail = useQuery({
		queryKey: transcriptDocKeys.detail(key),
		queryFn: () => fetchTranscriptDoc(key),
	});
	const pending = (detail.data?.segments ?? []).some((s) =>
		isPending(s.status),
	);
	const status = useQuery({
		queryKey: transcriptDocKeys.status(key),
		queryFn: () => fetchTranscriptStatus(key),
		enabled: pending,
		refetchInterval: STATUS_POLL_MS,
	});

	const segments = detail.data?.segments;
	const statuses = status.data;
	const statusAt = status.dataUpdatedAt;
	const detailAt = detail.dataUpdatedAt;
	useEffect(() => {
		// Only statuses newer than the loaded transcript can say it is stale.
		if (!segments || !statuses || statusAt <= detailAt) return;
		if (needsReload(segments, statuses)) {
			void queryClient.invalidateQueries({
				queryKey: transcriptDocKeys.detail(key),
			});
			void queryClient.invalidateQueries({ queryKey: transcriptDocKeys.list });
		}
	}, [segments, statuses, statusAt, detailAt, key, queryClient]);

	return { detail, statuses: pending ? statuses : undefined };
}

function useInvalidateDocs() {
	const queryClient = useQueryClient();
	return () =>
		queryClient.invalidateQueries({ queryKey: transcriptDocKeys.all });
}

export function useRenameTranscript() {
	const invalidate = useInvalidateDocs();
	return useMutation({
		mutationFn: ({ key, title }: { key: string; title: string }) =>
			renameTranscript(key, title),
		onSettled: invalidate,
	});
}

export function useSaveSegment() {
	const invalidate = useInvalidateDocs();
	return useMutation({
		mutationFn: (input: {
			key: string;
			fileId: string;
			runId: string;
			texts: readonly string[];
		}) => saveSegmentTexts(input.key, input.fileId, input.runId, input.texts),
		onSettled: invalidate,
	});
}

export function useDeleteTranscript() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (key: string) => deleteTranscript(key),
		onSuccess: (_, key) => {
			queryClient.removeQueries({ queryKey: transcriptDocKeys.detail(key) });
		},
		onSettled: () =>
			Promise.all([
				queryClient.invalidateQueries({ queryKey: transcriptDocKeys.list }),
				// The recordings list and its "Chép lời" panels show transcript states too.
				queryClient.invalidateQueries({ queryKey: fileKeys.recordings }),
				queryClient.invalidateQueries({ queryKey: transcriptKeys.all }),
			]),
	});
}
