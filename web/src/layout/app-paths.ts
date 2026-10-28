/** URL of each page of the signed-in app. */
export const PATHS = {
	record: "/record",
	split: "/split",
	transcripts: "/transcripts",
	search: "/search",
	/** Public: no sign-in needed (App Store privacy and terms URLs). */
	privacy: "/privacy",
	terms: "/terms",
	/** Public: the sign-in card, for visitors only. */
	login: "/login",
} as const;

/**
 * One transcript. Keys of an iPhone session contain ':' (`s:<id>`), so the
 * key is encoded; the router decodes it again for `useParams`. `at` (seconds
 * of the session) makes the reader jump there and play (`?t=`).
 */
export function transcriptPath(key: string, at?: number): string {
	const path = `${PATHS.transcripts}/${encodeURIComponent(key)}`;
	if (at === undefined || !Number.isFinite(at) || at < 0) return path;
	return `${path}?t=${Math.round(at * 10) / 10}`;
}

/** Seconds of a `?t=` value, or undefined when missing or not a time. */
export function parseStartAt(raw: string | null): number | undefined {
	if (raw === null || raw.trim() === "") return undefined;
	const at = Number(raw);
	return Number.isFinite(at) && at >= 0 ? at : undefined;
}

/** The search page; `query` fills its field (`?q=`). */
export function searchPath(query = ""): string {
	const q = query.trim();
	return q
		? `${PATHS.search}?${new URLSearchParams({ q }).toString()}`
		: PATHS.search;
}
