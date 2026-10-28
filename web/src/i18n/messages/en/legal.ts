import type { SourceMessages, Translation } from "../../message-types";

/**
 * English of `vi/legal.ts`: a full translation of the privacy policy and
 * the terms. The Vietnamese text is the one that applies (`authoritative`,
 * shown at the top of the English page). App labels quoted in the text are
 * the English UI's. `english*` is the short summary the Vietnamese page
 * ends with; the English page does not show it.
 */
export const legal: Translation<SourceMessages["legal"]> = {
	effective: "Effective from {date}",
	nav: {
		label: "Legal",
		privacy: "Privacy",
		terms: "Terms",
	},
	contactFallback: "the Scribe operator through your {idpName} account",
	englishTitle: "English summary",
	authoritative: {
		title: "About this translation",
		body: "This English text is a translation for your convenience. The Vietnamese version is the one that applies; if the two differ, the Vietnamese version prevails.",
	},
	privacy: {
		title: "Privacy Policy",
		intro:
			"Scribe (the website scribe.hoandat.com and the Scribe app on iPhone) records, splits files and transcribes meetings in Vietnamese. This policy explains what data Scribe collects, what it is used for, who it is sent to, how long it is kept and what rights you have. “We” means the operator of Scribe.",
		collect: {
			title: "Data we collect",
			body: "- <b>Account:</b> the email, display name and account identifier of your {idpName} account that auth.hoandat.com sends to Scribe when you sign in; your current plan.\n- <b>Content you create:</b> audio recordings, screen recordings, files you upload, transcripts, sentences you edit, speaker names, titles, minutes, bookmarks and translations.\n- <b>“Ask Scribe” questions:</b> used only to answer and then discarded, not stored.\n- <b>Your API keys:</b> API keys you enter yourself, stored encrypted (AES-256-GCM); Scribe only shows the last 4 characters and never sends a key back to the browser or the app.\n- <b>Usage:</b> storage, transcription minutes, and the number of summaries, questions and translations.\n- <b>Security logs:</b> time, action (sign-in, upload, deletion…) and IP address; the server's technical access logs.\n- <b>On iPhone:</b> recorded parts stay on the device until they finish uploading; the device's sign-in token is kept in the Keychain. The app uses the microphone only while you record, the camera and photo library only when you choose to film or pick a file to upload, and writes to the Photos library only when you choose “Save Video” in the share sheet; it does not read your contacts or location.\n\nScribe has no ads, uses no tracking or analytics SDKs in the app, does not sell data and does not track you across other apps or websites. The website scribe.hoandat.com uses Cloudflare Web Analytics to count page views; it sets no cookies and stores nothing on your device.",
		},
		purposes: {
			title: "What we use data for",
			body: "- To provide the features you use: recording, storing, playing, splitting, transcribing, live transcription, minutes, search, questions, translation, file export and sending minutes to chat.\n- To apply your plan's limits and prevent abuse.\n- To keep your account safe and handle incidents.\n\nWe do not use your content to train models and we do not read your content, unless you ask for support with a specific recording.",
		},
		processors: {
			title: "Data processors",
			body: "Data is sent only to the parties below, and only to do what you ask:\n\n- <b>Scribe's server</b> (a rented server in Vietnam): stores files, transcripts and the database.\n- <b>Cloudflare</b>: relays traffic and encrypts HTTPS between your device and the server. <cloudflare>Cloudflare's policy</cloudflare>.\n- <b>ElevenLabs</b>: receives audio when you press Transcribe, and live audio when you turn on Live transcription on iPhone. <elevenlabs>ElevenLabs' policy</elevenlabs>.\n- <b>OpenAI</b>: currently the server's text model provider. It receives transcript text when you press Summarise, Ask Scribe or Translate, and receives audio when the server uses OpenAI as its fallback transcription route. <openai>OpenAI's policy</openai>. The server may be configured to use Google Gemini or Anthropic Claude instead of OpenAI for this work; if so, this policy is updated first.\n- <b>Providers of your API keys:</b> when you add your own key (OpenAI, Google Gemini, Anthropic Claude or ElevenLabs), the matching work runs on your account with that provider, under the terms you accepted with them. <google>Google's policy</google>, <anthropic>Anthropic's policy</anthropic>.\n- <b>Your {idpName} account</b> (auth.hoandat.com): sign-in.\n- <b>chat.hoandat.com</b>: only when you link chat; it receives the minutes you choose to send. When an account hits the free limit several times, the account's email is sent to the operator's private alert channel on chat.hoandat.com.",
		},
		retention: {
			title: "How long we keep data",
			body: "- <b>Files and everything made from them</b> (transcripts, edited sentences, minutes, bookmarks, translations, search index) are deleted automatically 7 days after creation on the free plan and 14 days after creation on the Plus plan. Scribe is a temporary workspace, not an archive: download what you need to keep.\n- <b>Account, your API keys, chat link:</b> until you delete them.\n- <b>Security logs:</b> 180 days.\n- <b>Monthly usage figures:</b> until you delete your account. After deletion, only anonymised cost figures remain (no longer tied to you or to any recording), to reconcile the service's costs.\n- <b>Backups:</b> before some server updates, the operator copies the database (not the audio files) to a backup folder on the same server to restore it if something goes wrong; these copies are deleted once the update is stable. Data you have deleted may remain in such a backup until it is deleted.\n- Processors keep data under their own policies.",
		},
		rights: {
			title: "Your rights",
			body: "- <b>View and download:</b> play and download each recording, “Download all” in Recording history, export transcripts as TXT, SRT, VTT, Markdown or Word.\n- <b>Edit:</b> edit sentences, titles and speaker names.\n- <b>Delete:</b> delete individual recordings, “Delete all”, delete your API keys, unlink chat.\n- <b>Delete your account:</b> choose “Delete account” in the account menu. Scribe immediately deletes all your files, transcripts, minutes, translations, bookmarks, search index, API keys, chat link, sign-in sessions and usage figures. Security logs are kept until the end of the 180 days but no longer contain your email, file names or error details; cost figures are kept in anonymised form. Your {idpName} account is shared by several apps; delete it at <account>auth.hoandat.com/account</account>.\n- Other data requests (for example a copy of your account data): contact us as described under Contact.",
		},
		security: {
			title: "Security",
			body: "Connections use HTTPS; sign-in sessions use HttpOnly cookies and can be revoked; your API keys are encrypted; each account sees only its own data. No system is perfectly secure; if we discover an incident affecting your data, we will tell you.",
		},
		children: {
			title: "Children",
			body: "Scribe is not meant for children under 13 and does not knowingly collect data from children under 13. People from 13 to under 16 may use it only with the consent of a parent or guardian. If you know that a child has used Scribe, contact us so we can delete the data.",
		},
		changes: {
			title: "Changes to this policy",
			body: "When the policy changes, the effective date at the top of the page changes with it; important changes are announced in the app before they apply.",
		},
		english:
			"Scribe records meetings and transcribes Vietnamese. We collect your {idpName} account email and name, the recordings and files you create, their transcripts and the things you make from them (edits, minutes, bookmarks, translations), your own API keys (encrypted, never shown back), usage counts, and security logs (time, action, IP address). Audio goes to ElevenLabs for transcription; transcript text goes to OpenAI (the server's current text model, which may be switched to Google Gemini or Anthropic Claude after updating this policy) for minutes, Q&A and translation, or to your own provider when you add a key; Cloudflare carries the traffic. The app writes to your Photos library only when you choose “Save Video”. No ads, no tracking or analytics SDKs in the app, no sale of data, no model training on your content; the website counts page views with Cloudflare Web Analytics, which sets no cookies. Files and everything made from them are deleted automatically after 7 days (free) or 14 days (Plus); security logs are kept 180 days; database backups taken on the server before some updates are removed once the update is stable; after account deletion only anonymised cost figures remain. You can download, edit and delete your data, and delete your Scribe account in the app (account menu, “Delete account”); your {idpName} account is deleted at auth.hoandat.com/account. Scribe is not for children under 13.",
	},
	terms: {
		title: "Terms of Use",
		intro:
			"By using Scribe (the website scribe.hoandat.com and the Scribe app on iPhone), you agree to the terms below and to the <privacy>Privacy Policy</privacy>.",
		account: {
			title: "Account",
			body: "You sign in with your {idpName} account and are responsible for keeping it safe. Each account is for one person. You must be 16 or older, or 13 or older with the consent of a parent or guardian.",
		},
		plans: {
			title: "Plans and limits",
			body: "The free plan has 200 MB of storage and keeps files for 7 days; the Plus plan has 5 GB and keeps files for 14 days. Your plan's transcription minutes and number of summaries, questions and translations are shown on the “Plan and usage” card. Limits may change; when the service's shared limit is used up, some features pause until the next month.",
		},
		storage: {
			title: "Scribe is not an archive",
			body: "Files and everything made from them are deleted automatically when your plan's retention period ends. Download and keep anything you need long-term yourself; we cannot recover deleted data.",
		},
		content: {
			title: "Your content",
			body: "You keep all rights to your content. You allow Scribe to store and process your content and send it to the processors named in the Privacy Policy, only to provide the features you use.",
		},
		use: {
			title: "Acceptable use",
			body: "- Ask for and obtain the consent of the people you record on audio or video, as the law requires; do not record secretly.\n- Do not upload unlawful content or content that infringes the rights of others.\n- Do not create several accounts to get around limits, and do not scan, attack or overload the service.",
		},
		ai: {
			title: "Machine-generated results",
			body: "Transcripts, speaker names, minutes, translations and “Ask Scribe” answers are generated by machine models and may be wrong. Check them before using them for anything important.",
		},
		keys: {
			title: "Your API keys",
			body: "Work run on your key is billed to your account with that provider, under their terms. You can delete your key at any time.",
		},
		service: {
			title: "The service",
			body: "Scribe is provided as is. The service may be interrupted, changed or discontinued; we do not guarantee that it is always available or free of errors. To the extent permitted by law, we are not liable for indirect damages or for the loss of data whose retention period has ended.",
		},
		end: {
			title: "Termination",
			body: "You can delete your Scribe account at any time from the account menu. We may block accounts that break these terms.",
		},
		law: {
			title: "Governing law",
			body: "These terms are governed by the laws of Vietnam.",
		},
		changes: {
			title: "Changes to these terms",
			body: "When the terms change, the effective date at the top of the page changes with them; important changes are announced in the app before they apply.",
		},
		english:
			"By using Scribe you accept these terms and the Privacy Policy. Sign in with your {idpName} account and keep it safe; one account per person; you must be 16 or older, or 13 or older with a parent's consent. Free plan: 200 MB, files kept 7 days; Plus: 5 GB, 14 days; other limits are shown in the app and may change. Scribe is a workspace, not an archive: download what you need to keep. You own your content and let Scribe process it with the providers named in the Privacy Policy only to run the features you use. Get the consent of the people you record as the law requires; no unlawful content, no abuse. Transcripts, minutes, translations and answers are machine-made and may be wrong. Work run on your own API keys is billed by your provider. The service is provided as is and may change or stop. You can delete your Scribe account in the app at any time; accounts that break these terms may be blocked. Vietnamese law applies.",
	},
	contact: {
		title: "Contact",
		body: "Questions or requests about your data and these terms: <contact>{contact}</contact>.",
	},
};
