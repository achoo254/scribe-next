import { keepPreviousData, useMutation, useQuery } from "@tanstack/react-query";
import { queryWords } from "./normalize";
import { type SearchReply, askScribe, searchTranscripts } from "./search-api";

export const searchKeys = {
	all: ["search"] as const,
	query: (query: string) => ["search", query] as const,
};

/** Shortest lone word searched (the backend's prefix search needs as much). */
export const MIN_WORD_CHARS = 2;

/** Whether `query` is searched: some word, and a lone word of at least {@link MIN_WORD_CHARS}. */
export function isSearchable(query: string): boolean {
	const words = queryWords(query);
	return words.length > 1 || (words[0]?.length ?? 0) >= MIN_WORD_CHARS;
}

/** Search results with the query they answer (kept on screen while the next query loads). */
export interface SearchReplyFor extends SearchReply {
	query: string;
}

/** Results of `query` (already debounced); idle while the query is not searchable. */
export function useSearchQuery(query: string) {
	const trimmed = query.trim();
	return useQuery({
		queryKey: searchKeys.query(trimmed),
		queryFn: async ({ signal }): Promise<SearchReplyFor> => ({
			...(await searchTranscripts(trimmed, {}, signal)),
			query: trimmed,
		}),
		enabled: isSearchable(trimmed),
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
