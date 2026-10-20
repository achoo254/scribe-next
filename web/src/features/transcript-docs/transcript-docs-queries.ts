import {
	skipToken,
	useMutation,
	useQuery,
	useQueryClient,
} from "@tanstack/react-query";
import { useEffect } from "react";
import { ApiError } from "../../lib/api";
import { fileKeys } from "../files/file-queries";
import { transcriptKeys } from "../transcript/transcript-queries";
import { isPending, needsReload } from "./reader-model";
import {
	type TranscriptDetail,
	type TranscriptSummary,
	type TranscriptTranslation,
	type TranslationLang,
	type TranslationProgress,
	createSummary,
	deleteBookmark,
	deleteTranscript,
	fetchTranscriptDoc,
	fetchTranscriptStatus,
	fetchTranslation,
	fetchTranslationStatus,
	listTranscriptDocs,
	renameSpeakers,
	renameTranscript,
	saveSegmentTexts,
	shareSummary,
	startTranslation,
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
	translation: (key: string, lang: TranslationLang) =>
		["transcript-docs", "translation", key, lang] as const,
	translationStatus: (key: string, lang: TranslationLang) =>
		["transcript-docs", "translation-status", key, lang] as const,
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

export function useRenameSpeakers() {
	const invalidate = useInvalidateDocs();
	return useMutation({
		mutationFn: ({
			key,
			names,
		}: {
			key: string;
			names: Readonly<Record<string, string>>;
		}) => renameSpeakers(key, names),
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

/**
 * The reply was lost rather than refused (no network, 5xx, a proxy timeout
 * such as 524): the server may still have saved the minutes.
 */
export function replyMayBeLost(error: unknown): boolean {
	return (
		!(error instanceof ApiError) || error.status === 0 || error.status >= 500
	);
}

/**
 * "Tóm tắt" / "Tóm tắt lại": the new minutes replace the old in the open
 * doc. The server finishes the minutes even when the reply is lost, so the
 * doc is read again then, to show them once saved.
 */
export function useCreateSummary(key: string) {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: () => createSummary(key),
		onSuccess: (summary) => {
			queryClient.setQueryData<TranscriptDetail>(
				transcriptDocKeys.detail(key),
				(doc) => (doc ? { ...doc, summary } : doc),
			);
		},
		onError: (error) => {
			if (!replyMayBeLost(error)) return;
			void queryClient.invalidateQueries({
				queryKey: transcriptDocKeys.detail(key),
			});
		},
	});
}

export function useShareSummary(key: string) {
	return useMutation({ mutationFn: () => shareSummary(key) });
}

export function useDeleteBookmark(key: string) {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (id: string) => deleteBookmark(key, id),
		onSuccess: (_, id) => {
			queryClient.setQueryData<TranscriptDetail>(
				transcriptDocKeys.detail(key),
				(doc) =>
					doc
						? {
								...doc,
								bookmarks: (doc.bookmarks ?? []).filter((b) => b.id !== id),
							}
						: doc,
			);
		},
		onSettled: () =>
			queryClient.invalidateQueries({
				queryKey: transcriptDocKeys.detail(key),
			}),
	});
}

/**
 * The translation of a transcript into `lang` (none while `lang` is null).
 * While a run is going, only the light `/translation/status` route is
 * polled; the sentences reload once the run ends (done or failed).
 */
export function useTranslation(key: string, lang: TranslationLang | null) {
	const queryClient = useQueryClient();
	const translation = useQuery({
		queryKey: transcriptDocKeys.translation(key, lang ?? "en"),
		queryFn: lang ? () => fetchTranslation(key, lang) : skipToken,
	});
	const running = lang !== null && translation.data?.status === "running";
	const status = useQuery({
		queryKey: transcriptDocKeys.translationStatus(key, lang ?? "en"),
		queryFn: lang ? () => fetchTranslationStatus(key, lang) : skipToken,
		enabled: running,
		refetchInterval: STATUS_POLL_MS,
	});

	const progress = status.data;
	useEffect(() => {
		// The loaded sentences say running, the live status says the run ended.
		if (!lang || !running || !progress || progress.status === "running") {
			return;
		}
		void queryClient.invalidateQueries({
			queryKey: transcriptDocKeys.translation(key, lang),
		});
		void queryClient.invalidateQueries({
			queryKey: transcriptDocKeys.detail(key),
		});
	}, [lang, running, progress, key, queryClient]);

	const data: TranscriptTranslation | undefined = lang
		? translation.data
		: undefined;
	return {
		translation: data,
		/** Live progress while running, else the loaded translation's own. */
		progress: running && progress ? progress : data,
		query: translation,
	};
}

/**
 * "Dịch" into a language: the reply (`running`, or `done` when nothing was
 * left) shows at once; the sentences and the transcript's list of
 * translations reload from the server.
 */
export function useStartTranslation(key: string) {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (lang: TranslationLang) => startTranslation(key, lang),
		onSuccess: async (progress: TranslationProgress, lang) => {
			// A status left from an earlier run must not end this one.
			queryClient.removeQueries({
				queryKey: transcriptDocKeys.translationStatus(key, lang),
			});
			// Picking a new language loads its translation while the POST is
			// sent. That load may have been answered before the run started
			// ("none"); a query with no data yet would only wait for it on
			// invalidation, so it is dropped and asked again.
			await queryClient.cancelQueries({
				queryKey: transcriptDocKeys.translation(key, lang),
			});
			queryClient.setQueryData<TranscriptTranslation>(
				transcriptDocKeys.translation(key, lang),
				(old) => (old ? { ...old, ...progress } : old),
			);
			void queryClient.invalidateQueries({
				queryKey: transcriptDocKeys.translation(key, lang),
			});
			void queryClient.invalidateQueries({
				queryKey: transcriptDocKeys.detail(key),
			});
		},
	});
}
