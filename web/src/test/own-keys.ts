import type { OwnKeys } from "../auth/auth-api";

/** `ownKeys` of a user with a valid key for every AI use (ElevenLabs + OpenAI). */
export const ALL_OWN_KEYS: OwnKeys = {
	available: true,
	transcribe: "elevenlabs",
	live: "elevenlabs",
	text: "openai",
	textModel: null,
	minutes: 0,
};

/** `ownKeys` of a user without any key: every AI use is unavailable. */
export const NO_OWN_KEYS: OwnKeys = {
	available: true,
	transcribe: null,
	live: null,
	text: null,
	textModel: null,
	minutes: 0,
};
