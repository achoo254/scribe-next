import { isNativeShell } from "../../lib/api";
import { Secrets } from "../../lib/native";

/**
 * The user's own OpenAI key. The iOS app keeps it in its Keychain; the web
 * keeps it for this tab only (sessionStorage), so a shared computer does
 * not remember it. It reaches the backend only with a transcription request.
 */
const NAME = "openaiApiKey";

export async function loadApiKey(): Promise<string> {
	if (isNativeShell()) {
		const { value } = await Secrets.get({ name: NAME });
		return value ?? "";
	}
	try {
		return sessionStorage.getItem(NAME) ?? "";
	} catch {
		return "";
	}
}

export async function saveApiKey(value: string): Promise<void> {
	if (isNativeShell()) {
		await Secrets.set({ name: NAME, value });
		return;
	}
	try {
		sessionStorage.setItem(NAME, value);
	} catch {
		// Storage blocked (private mode): the key is used for this request only.
	}
}

export async function forgetApiKey(): Promise<void> {
	if (isNativeShell()) {
		await Secrets.remove({ name: NAME });
		return;
	}
	try {
		sessionStorage.removeItem(NAME);
	} catch {
		// Nothing stored.
	}
}

/** Same check as the backend, to answer before a round trip. */
export function looksLikeApiKey(value: string): boolean {
	return /^sk-\S{10,}$/.test(value.trim());
}
