import type { SourceMessages, Translation } from "../../message-types";

/** English of `vi/landing.ts`: the same keys, the same facts. */
export const landing: Translation<SourceMessages["landing"]> = {
	nav: {
		features: "Features",
		how: "How it works",
	},
	hero: {
		eyebrow: "Record and transcribe Vietnamese",
		title: "Talk. Get the transcript.",
		lede: "Scribe records right in your browser or on iPhone, splits long audio and video files into parts, then transcribes the Vietnamese speaker by speaker so you can search, edit and download it.",
	},
	signIn: {
		title: "Try it with your next meeting",
		body: "The free plan has 200 MB of storage; files are deleted after 7 days, so download what you want to keep.",
	},
	features: {
		eyebrow: "Features",
		title: "One place for the whole meeting",
		record: {
			title: "Record on the web and iPhone",
			body: "Record the microphone, or your computer's sound and screen, right in the browser. On iPhone, Scribe keeps recording while the screen is locked.",
		},
		split: {
			title: "Split audio and video files",
			body: "Upload a long file, split it by minutes, by size or into equal parts, then download the parts as a zip.",
		},
		transcribe: {
			title: "Vietnamese transcripts",
			body: "Transcripts are split by speaker, with timestamps to listen again. Fix sentences, name the speakers, export TXT, SRT, VTT, Markdown or Word.",
		},
		after: {
			title: "Minutes, search, questions",
			body: "Summarise into meeting minutes, search every transcript, ask Scribe and get answers with timestamps, translate transcripts between Vietnamese and English.",
		},
	},
	how: {
		eyebrow: "How it works",
		title: "Three steps to a transcript",
		steps: {
			signIn: {
				title: "Sign in",
				body: "Use your {idpName} account, no new password needed.",
			},
			capture: {
				title: "Record or upload",
				body: "Record on the web or on iPhone, or pick an audio or video file you already have.",
			},
			read: {
				title: "Get the transcript",
				body: "Press Transcribe, then read, edit, search and download it when it is done.",
			},
		},
	},
};
