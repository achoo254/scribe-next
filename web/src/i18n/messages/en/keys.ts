import type { SourceMessages, Translation } from "../../message-types";

/** English of `vi/keys.ts`: the user's own API keys. */
export const keys: Translation<SourceMessages["keys"]> = {
	menu: "Your API keys",
	dialog: {
		title: "Your API keys",
		intro:
			"Use your own API keys instead of Scribe's. Keys are encrypted and stored on the Scribe server and never shown again; the provider bills your account, and work run on your keys does not count against Scribe's minutes or requests.",
		unavailable:
			"Your API keys are not enabled on this server. An admin needs to create the encryption secret file and restart.",
		loading: "Loading your API keys…",
		saved: "••••{last4} · checked {date}",
		none: "No key yet",
		status: {
			invalid: "The provider rejected this key: enter a new key or delete it.",
			quota:
				"The key's account is out of credit: top it up or enter another key.",
			unreadable: "The server can't read the saved key: enter it again.",
		},
		field: "{provider} key",
		placeholderNew: "Paste key",
		placeholderReplace: "Paste a new key to replace it",
		save: "Save and check",
		delete: "Delete key",
		savedToast: "{provider} key checked and saved",
		deletedToast: "{provider} key deleted",
		create: "Create a key at {provider}",
		opensBrowser: ", opens in a new tab",
	},
	provider: {
		elevenlabs: {
			name: "ElevenLabs",
			usedFor:
				"Transcribing files and whole iPhone sessions (with speakers), live transcription. The key needs Speech to Text access.",
		},
		openai: {
			name: "OpenAI",
			usedFor:
				"Minutes, questions, translation; transcribing files (gpt-transcribe, no speakers); live transcription.",
		},
		gemini: {
			name: "Google Gemini",
			usedFor:
				"Minutes, questions, translation; transcribing files (no speakers, sentence times are estimates). Not used for live transcription.",
		},
		anthropic: {
			name: "Anthropic Claude",
			usedFor: "Minutes, questions, translation. Claude does not transcribe.",
		},
	},
	uses: {
		title: "Which key does what",
		hint: "Only providers with a valid key can be chosen.",
		transcribe: "Transcribing files",
		live: "Live transcription on iPhone",
		text: "Minutes, questions, translation",
		scribe: "Using Scribe's key, counted in your plan's usage.",
		needsKey: "{provider} (no key yet)",
		model: "Model",
		geminiNote: "Gemini: sentence times are estimates, no speakers.",
	},
};
