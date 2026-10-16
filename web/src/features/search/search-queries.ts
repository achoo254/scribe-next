import { keepPreviousData, useMutation, useQuery } from "@tanstack/react-query";
import { askScribe, searchTranscripts } from "./search-api";

export const searchKeys = {
	all: ["search"] as const,
	query: (query: string) => ["search", query] as const,
};

/** Results of `query` (already debounced); idle while the query is blank. */
export function useSearchQuery(query: string) {
	const trimmed = query.trim();
	return useQuery({
		queryKey: searchKeys.query(trimmed),
		queryFn: ({ signal }) => searchTranscripts(trimmed, {}, signal),
		enabled: trimmed.length > 0,
		// Typing on keeps the last results on screen until the next arrive.
		placeholderData: keepPreviousData,
		staleTime: 30_000,
	});
}

/** One question to Scribe, on every transcript or on `key`. */
export function useAsk(key?: string) {
	return useMutation({
		mutationFn: (question: string) => askScribe(question, key),
	});
}
