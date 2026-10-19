import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useAuth } from "../../auth/auth-context";
import {
	type KeyChoice,
	type KeyProvider,
	type StoredKey,
	type UserKeys,
	deleteUserKey,
	fetchUserKeys,
	saveKeyChoice,
	saveUserKey,
} from "./user-keys-api";

export const userKeysKeys = { all: ["user-keys"] as const };

export function useUserKeysQuery(enabled = true) {
	return useQuery({
		queryKey: userKeysKeys.all,
		queryFn: () => fetchUserKeys(),
		enabled,
	});
}

/**
 * After any change the list is read again (which uses are now on the
 * user's keys) and so is `/api/me`, whose `ownKeys` the usage card shows.
 */
function useRefreshAfterChange() {
	const queryClient = useQueryClient();
	const { refresh } = useAuth();
	return () => {
		void queryClient.invalidateQueries({ queryKey: userKeysKeys.all });
		void refresh();
	};
}

export function useSaveUserKey() {
	const queryClient = useQueryClient();
	const refreshAll = useRefreshAfterChange();
	return useMutation({
		mutationFn: ({ provider, key }: { provider: KeyProvider; key: string }) =>
			saveUserKey(provider, key),
		onSuccess: (stored: StoredKey) => {
			queryClient.setQueryData<UserKeys>(userKeysKeys.all, (previous) =>
				previous
					? {
							...previous,
							keys: [
								...previous.keys.filter((k) => k.provider !== stored.provider),
								stored,
							],
						}
					: previous,
			);
			refreshAll();
		},
	});
}

export function useDeleteUserKey() {
	const queryClient = useQueryClient();
	const refreshAll = useRefreshAfterChange();
	return useMutation({
		mutationFn: (provider: KeyProvider) => deleteUserKey(provider),
		onSuccess: (_result, provider: KeyProvider) => {
			queryClient.setQueryData<UserKeys>(userKeysKeys.all, (previous) =>
				previous
					? {
							...previous,
							keys: previous.keys.filter((k) => k.provider !== provider),
						}
					: previous,
			);
			refreshAll();
		},
	});
}

export function useSaveKeyChoice() {
	const queryClient = useQueryClient();
	const { refresh } = useAuth();
	return useMutation({
		mutationFn: (choice: KeyChoice) => saveKeyChoice(choice),
		onSuccess: (keys: UserKeys) => {
			queryClient.setQueryData(userKeysKeys.all, keys);
			void refresh();
		},
	});
}
