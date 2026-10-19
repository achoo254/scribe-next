import type { KeyProvider } from "../features/user-keys/user-keys-api";
import type { MessageKey } from "../i18n/message-types";

/** Name and use of each provider, and where its keys are made. */
export const PROVIDER_INFO = {
	elevenlabs: {
		name: "keys.provider.elevenlabs.name",
		usedFor: "keys.provider.elevenlabs.usedFor",
		createUrl: "https://elevenlabs.io/app/settings/api-keys",
	},
	openai: {
		name: "keys.provider.openai.name",
		usedFor: "keys.provider.openai.usedFor",
		createUrl: "https://platform.openai.com/api-keys",
	},
	gemini: {
		name: "keys.provider.gemini.name",
		usedFor: "keys.provider.gemini.usedFor",
		createUrl: "https://aistudio.google.com/apikey",
	},
	anthropic: {
		name: "keys.provider.anthropic.name",
		usedFor: "keys.provider.anthropic.usedFor",
		createUrl: "https://console.anthropic.com/settings/keys",
	},
} as const satisfies Record<
	KeyProvider,
	{ name: MessageKey; usedFor: MessageKey; createUrl: string }
>;
