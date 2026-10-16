import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";
import {
	type ChatLinkStatus,
	fetchChatLink,
	startChatLink,
	unlinkChat,
} from "./chat-link-api";

export const chatLinkKeys = { status: ["chat-link"] as const };

/** How often the status is read while a link waits for "Xác nhận liên kết". */
export const LINK_POLL_MS = 3000;

/** The status once the link is gone (unlinked here, or dropped by the server). */
export function unlinkedStatus(
	previous: ChatLinkStatus | undefined,
): ChatLinkStatus {
	return { linked: false, linkedAt: null, enabled: previous?.enabled ?? true };
}

/**
 * Whether the account is linked to chat.hoandat.com. Linking finishes in
 * another tab or app (the system browser on iPhone), so the status is read
 * again whenever this page gets the focus back or becomes visible, and
 * every few seconds until `pollUntil` (ISO) while a link is pending.
 */
export function useChatLinkQuery(
	enabled = true,
	pollUntil: string | null = null,
) {
	const query = useQuery({
		queryKey: chatLinkKeys.status,
		queryFn: () => fetchChatLink(),
		enabled,
		refetchInterval: (current) => {
			if (!pollUntil || current.state.data?.linked) return false;
			return Date.now() < new Date(pollUntil).getTime() ? LINK_POLL_MS : false;
		},
		// The listeners below cover it, `focus` included (React Query only
		// listens to `visibilitychange`).
		refetchOnWindowFocus: false,
	});
	const { refetch } = query;
	useEffect(() => {
		if (!enabled) return;
		const recheck = () => {
			if (document.visibilityState === "visible") void refetch();
		};
		window.addEventListener("focus", recheck);
		document.addEventListener("visibilitychange", recheck);
		return () => {
			window.removeEventListener("focus", recheck);
			document.removeEventListener("visibilitychange", recheck);
		};
	}, [enabled, refetch]);
	return query;
}

/** One-time deep link to open on chat.hoandat.com. */
export function useStartChatLink() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: () => startChatLink(),
		// CHAT_LINK_DISABLED: the server turned linking off meanwhile.
		onError: () =>
			queryClient.invalidateQueries({ queryKey: chatLinkKeys.status }),
	});
}

export function useUnlinkChat() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: () => unlinkChat(),
		onSuccess: () =>
			queryClient.setQueryData<ChatLinkStatus>(
				chatLinkKeys.status,
				unlinkedStatus,
			),
		onSettled: () =>
			queryClient.invalidateQueries({ queryKey: chatLinkKeys.status }),
	});
}
