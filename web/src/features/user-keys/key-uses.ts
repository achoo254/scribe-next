/*
 * Providers and uses of "Key riêng", free of app imports so the e2e stubs
 * (Node, no `import.meta.env`) can share them.
 */

export type KeyProvider = "elevenlabs" | "openai" | "gemini" | "anthropic";

export const KEY_PROVIDERS: readonly KeyProvider[] = [
	"elevenlabs",
	"openai",
	"gemini",
	"anthropic",
];

/** valid: checked on save, not refused since; the others need a fix. */
export type KeyStatus = "valid" | "invalid" | "quota" | "unreadable";

export interface StoredKey {
	provider: KeyProvider;
	last4: string;
	status: KeyStatus;
	updatedAt: string | null;
	lastCheckedAt: string | null;
}

/** What a key can be used for, and which providers can do it. */
export type KeyUse = "transcribe" | "live" | "text";

export const USE_PROVIDERS: Record<KeyUse, readonly KeyProvider[]> = {
	transcribe: ["elevenlabs", "openai", "gemini"],
	live: ["elevenlabs", "openai"],
	text: ["openai", "gemini", "anthropic"],
};
