import type { SourceMessages, Translation } from "../../message-types";

/** English of `vi/split.ts`: the Split file page. */
export const split: Translation<SourceMessages["split"]> = {
	title: "Split file",
	description:
		"Upload an audio or video file, choose how to split it, then download the parts as a zip.",
	steps: {
		file: "Choose file",
		criteria: "How to split",
	},
	choose: "Choose an audio/video file",
	chooseAnother: "Choose another file",
	uploadProgress: "Upload progress",
	uploading: "Uploading… {percent}%",
	resume: "Resume upload",
	uploaded: "Uploaded, length {duration}",
	start: "Split",
	working: "Processing…",
	result: { one: "Result ({count} part)", other: "Result ({count} parts)" },
	downloadZip: "Download zip",
	criteria: {
		label: "Split by",
		byMinutes: "Length (minutes)",
		byMegabytes: "Size (MB)",
		byParts: "Equal parts",
		minutesValue: "Minutes per part",
		megabytesValue: "MB per part",
		partsValue: "Number of parts",
		valueInvalid: "Enter a number above 0.",
		partsInvalid: "Enter a whole number, 1 or more.",
	},
	managed: {
		title: "Files on the server",
		description:
			"Files you uploaded to split and the split parts still count toward your storage. Delete some to free space (deleted automatically after 14 days if you don't).",
		loadFailed: "Couldn't load the file list",
		uploads: "Uploaded files",
		outputs: "Split parts",
		segments: { one: "{count} part", other: "{count} parts" },
		menu: "Actions for {name}",
		download: "Download",
		deleteAll: "Delete all",
		deleteTitle: "Delete files",
		deleteFile: "Delete “{name}”?",
		deleteSegments: {
			one: "Delete {count} split part?",
			other: "Delete {count} split parts?",
		},
		deleteBody: "The space is freed right away. This cannot be undone.",
		fileDeleted: "Deleted “{name}”",
		segmentsDeleted: {
			one: "Deleted {count} split part",
			other: "Deleted {count} split parts",
		},
	},
};
