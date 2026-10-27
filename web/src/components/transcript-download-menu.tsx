import {
	IconBadgeCc,
	IconDownload,
	IconFileText,
	IconFileTypeDocx,
	IconMarkdown,
} from "@tabler/icons-react";
import { type ReactNode, useState } from "react";
import type { TranslationLang } from "../features/transcript-docs/transcript-docs-api";
import {
	TRANSLATION_LANGS,
	TRANSLATION_LANG_LABEL,
} from "../features/transcript-docs/translation-langs";
import type {
	ExportFormat,
	ExportScope,
} from "../features/transcript/transcript-api";
import { useT } from "../i18n/i18n-context";
import { downloadPaths, useDownload } from "../lib/download";
import { Button } from "./ui/button";
import {
	DropdownMenu,
	DropdownMenuContent,
	DropdownMenuItem,
	DropdownMenuLabel,
	DropdownMenuRadioGroup,
	DropdownMenuRadioItem,
	DropdownMenuSeparator,
	DropdownMenuTrigger,
} from "./ui/dropdown-menu";

/** Menu entries in display order; format names read the same in every language. */
export const EXPORT_FORMATS: readonly {
	format: ExportFormat;
	label: string;
	icon: ReactNode;
}[] = [
	{
		format: "txt",
		label: "TXT",
		icon: <IconFileText size={16} aria-hidden="true" />,
	},
	{
		format: "srt",
		label: "SRT",
		icon: <IconBadgeCc size={16} aria-hidden="true" />,
	},
	{
		format: "vtt",
		label: "VTT",
		icon: <IconBadgeCc size={16} aria-hidden="true" />,
	},
	{
		format: "md",
		label: "Markdown",
		icon: <IconMarkdown size={16} aria-hidden="true" />,
	},
	{
		format: "docx",
		label: "Word (.docx)",
		icon: <IconFileTypeDocx size={16} aria-hidden="true" />,
	},
];

export interface TranscriptDownloadMenuProps {
	/** Files or iPhone session to export, joined in recording order. */
	scope: ExportScope;
	/** Name sent to the backend; a title the user gave the transcript wins. */
	name: string;
	/** `data-id` of the trigger; items get `<id>-<format>`, the menu `<id>-content`. */
	"data-id": string;
	/** Icon-only trigger on phone width (the label stays for screen readers), for crowded headers. */
	compact?: boolean;
	/** Stored translations of the transcript: the menu offers them beside the original. */
	languages?: readonly TranslationLang[];
}

const NO_LANGUAGES: readonly TranslationLang[] = [];

/**
 * "Tải xuống" button opening the export formats. The backend builds every
 * format, so the web and the iOS app (native download, then share sheet)
 * get the same file. A translated transcript also offers the language
 * (Gốc or a translation), applied to every format.
 */
export function TranscriptDownloadMenu({
	scope,
	name,
	"data-id": dataId,
	compact = false,
	languages = NO_LANGUAGES,
}: TranscriptDownloadMenuProps) {
	const t = useT();
	const download = useDownload();
	const [chosen, setChosen] = useState<TranslationLang | "original">(
		"original",
	);
	const offered = TRANSLATION_LANGS.filter((l) => languages.includes(l));
	// A translation removed meanwhile falls back to the original.
	const lang =
		chosen !== "original" && offered.includes(chosen) ? chosen : null;
	return (
		<DropdownMenu>
			<DropdownMenuTrigger asChild>
				<Button
					size="sm"
					variant="ghost"
					data-id={dataId}
					leadingIcon={<IconDownload size={16} aria-hidden="true" />}
					className="data-[state=open]:bg-fg/6 data-[state=open]:text-fg"
				>
					<span className={compact ? "max-sm:sr-only" : undefined}>
						{t("transcript.download.menu")}
					</span>
				</Button>
			</DropdownMenuTrigger>
			<DropdownMenuContent data-id={`${dataId}-content`}>
				{offered.length > 0 ? (
					<>
						<DropdownMenuLabel>
							{t("transcript.download.language")}
						</DropdownMenuLabel>
						<DropdownMenuRadioGroup
							value={lang ?? "original"}
							onValueChange={(value) =>
								setChosen(offered.find((l) => l === value) ?? "original")
							}
						>
							<DropdownMenuRadioItem
								value="original"
								data-id={`${dataId}-lang-original`}
								// Picking a language keeps the menu open for the format.
								onSelect={(event) => event.preventDefault()}
							>
								{t("transcript.download.original")}
							</DropdownMenuRadioItem>
							{offered.map((code) => (
								<DropdownMenuRadioItem
									key={code}
									value={code}
									data-id={`${dataId}-lang-${code}`}
									onSelect={(event) => event.preventDefault()}
								>
									{t(TRANSLATION_LANG_LABEL[code])}
								</DropdownMenuRadioItem>
							))}
						</DropdownMenuRadioGroup>
						<DropdownMenuSeparator />
					</>
				) : null}
				{EXPORT_FORMATS.map(({ format, label, icon }) => (
					<DropdownMenuItem
						key={format}
						data-id={`${dataId}-${format}`}
						icon={icon}
						onSelect={() =>
							download(
								downloadPaths.transcript(
									scope,
									format,
									name,
									lang ?? undefined,
								),
							)
						}
					>
						{label}
					</DropdownMenuItem>
				))}
			</DropdownMenuContent>
		</DropdownMenu>
	);
}
