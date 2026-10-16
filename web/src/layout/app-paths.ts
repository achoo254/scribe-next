/** URL of each page of the signed-in app. */
export const PATHS = {
	record: "/record",
	split: "/split",
	transcripts: "/transcripts",
} as const;

/**
 * One transcript. Keys of an iPhone session contain ':' (`s:<id>`), so the
 * key is encoded; the router decodes it again for `useParams`.
 */
export function transcriptPath(key: string): string {
	return `${PATHS.transcripts}/${encodeURIComponent(key)}`;
}
