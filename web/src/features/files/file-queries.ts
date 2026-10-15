import {
	type QueryClient,
	useMutation,
	useQuery,
	useQueryClient,
} from "@tanstack/react-query";
import {
	deleteFile,
	fetchUsage,
	listManagedFiles,
	listRecordings,
} from "./files-api";

export const fileKeys = {
	recordings: ["recordings"] as const,
	managed: ["managed-files"] as const,
	usage: ["usage"] as const,
};

export function useRecordingsQuery() {
	return useQuery({
		queryKey: fileKeys.recordings,
		queryFn: () => listRecordings(),
	});
}

export function useManagedFilesQuery() {
	return useQuery({
		queryKey: fileKeys.managed,
		queryFn: () => listManagedFiles(),
		// Opening the "Cắt file" tab shows what currently takes up quota.
		refetchOnMount: "always",
	});
}

/** Used/quota bytes; the bar stays hidden until the first answer. */
export function useUsageQuery() {
	return useQuery({ queryKey: fileKeys.usage, queryFn: () => fetchUsage() });
}

/** Re-reads what a new or removed file changes. */
export function refreshStorage(
	queryClient: QueryClient,
	lists: ReadonlyArray<"recordings" | "managed"> = [],
): Promise<void> {
	return Promise.all([
		queryClient.invalidateQueries({ queryKey: fileKeys.usage }),
		...lists.map((list) =>
			queryClient.invalidateQueries({ queryKey: fileKeys[list] }),
		),
	]).then(() => undefined);
}

/**
 * Deletes one or more files; every list and the quota refresh afterwards.
 * Resolves with how many were deleted, throws when none could be.
 */
export function useDeleteFiles() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: async (ids: readonly string[]) => {
			let deleted = 0;
			let lastError: unknown = null;
			for (const id of ids) {
				try {
					await deleteFile(id);
					deleted += 1;
				} catch (error) {
					lastError = error;
				}
			}
			if (deleted === 0 && lastError) throw lastError;
			return deleted;
		},
		onSettled: () => refreshStorage(queryClient, ["recordings", "managed"]),
	});
}
