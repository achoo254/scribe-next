import type { SourceMessages, Translation } from "../../message-types";

/** English of `vi/keys.ts`: the user's own API keys. */
export const keys: Translation<SourceMessages["keys"]> = {
	menu: "Your API keys",
	dialog: {
		title: "Your API keys",
		intro:
			"Transcription, live transcription, minutes, questions and translation run on your own API keys. Keys are encrypted and stored on the Scribe server, used only for your requests and never shown again; the provider bills your account.",
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
		off: "Off: add a valid key for one of the providers above.",
		needsKey: "{provider} (no key yet)",
		model: "Model",
		geminiNote: "Gemini: sentence times are estimates, no speakers.",
	},
	needed: {
		title: "Your API key is needed",
		open: "Open Your API keys",
		any: "Transcription, minutes, questions and translation run on your own API key (ElevenLabs, OpenAI, Google Gemini or Anthropic Claude). Add a key in Your API keys to use them.",
		transcribe:
			"Transcription runs on your own API key. Add an ElevenLabs, OpenAI or Google Gemini key in Your API keys.",
		live: "Live transcription runs on your own API key. Add an ElevenLabs or OpenAI key in Your API keys.",
		text: "Minutes, questions and translation run on your own API key. Add an OpenAI, Google Gemini or Anthropic Claude key in Your API keys.",
	},
};
