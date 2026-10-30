import { activeTranslator } from "../../i18n/active-translator";
import { apiJson } from "../../lib/api";

/**
 * Link between the account and the user's private chat with the Scribe bot
 * on chat.hoandat.com, where "Gửi sang chat" sends the minutes. Linking:
 * the backend hands out a one-time deep link; the user opens it and sends
 * `/start` to the bot, which the backend picks up by itself.
 */
export interface ChatLinkStart {
	/** `https://chat.hoandat.com/b/<bot>?start=<token>`. */
	url: string;
	/** The token stops working at this time (ISO). */
	expiresAt: string;
}

export interface ChatLinkStatus {
	linked: boolean;
	/** When the link was confirmed (ISO); null when not linked. */
	linkedAt: string | null;
	/**
	 * Name on chat.hoandat.com of whoever confirmed the link; null when not
	 * linked, or for links made before the server kept it.
	 */
	chatName: string | null;
	/** The server takes new links; false: linking is off on this server. */
	enabled: boolean;
}

/**
 * The link status. Linking completes only once the user presses "Xác nhận
 * liên kết" in the chat, so this is polled while a link is pending.
 */
export async function fetchChatLink(
	fetcher: typeof fetch = fetch,
): Promise<ChatLinkStatus> {
	const body = await apiJson<{
		linked?: unknown;
		linkedAt?: unknown;
		chatName?: unknown;
		enabled?: unknown;
	}>(
		"/api/chat/link",
		{ fallbackError: activeTranslator().t("errors.chat.status") },
		fetcher,
	);
	const linked = body.linked === true;
	return {
		linked,
		linkedAt:
			linked && typeof body.linkedAt === "string" && body.linkedAt
				? body.linkedAt
				: null,
		chatName:
			linked && typeof body.chatName === "string" && body.chatName.trim()
				? body.chatName.trim()
				: null,
		// Older servers do not say; they always took links.
		enabled: body.enabled !== false,
	};
}

/**
 * A new link. The bot words the linking steps (and its confirm button) in
 * the language on screen.
 */
export function startChatLink(
	fetcher: typeof fetch = fetch,
): Promise<ChatLinkStart> {
	return apiJson<ChatLinkStart>(
		"/api/chat/link",
		{
			method: "POST",
			json: { lang: activeTranslator().locale },
			fallbackError: activeTranslator().t("errors.chat.link"),
		},
		fetcher,
	);
}

export async function unlinkChat(fetcher: typeof fetch = fetch): Promise<void> {
	await apiJson<{ linked?: boolean }>(
		"/api/chat/link",
		{
			method: "DELETE",
			fallbackError: activeTranslator().t("errors.chat.unlink"),
		},
		fetcher,
	);
}
