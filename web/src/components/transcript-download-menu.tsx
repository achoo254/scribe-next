import {
	IconBadgeCc,
	IconDownload,
	IconFileText,
	IconFileTypeDocx,
	IconMarkdown,
} from "@tabler/icons-react";
import type { ReactNode } from "react";
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
	DropdownMenuTrigger,
} from "./ui/dropdown-menu";

/** Menu entries in display order; format names read the same in every language. */
const FORMATS: readonly {
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
}

/**
 * "Tải xuống" button opening the export formats. The backend builds every
 * format, so the web and the iOS app (native download, then share sheet)
 * get the same file.
 */
export function TranscriptDownloadMenu({
	scope,
	name,
	"data-id": dataId,
	compact = false,
}: TranscriptDownloadMenuProps) {
	const t = useT();
	const download = useDownload();
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
				{FORMATS.map(({ format, label, icon }) => (
					<DropdownMenuItem
						key={format}
						data-id={`${dataId}-${format}`}
						icon={icon}
						onSelect={() =>
							download(downloadPaths.transcript(scope, format, name))
						}
					>
						{label}
					</DropdownMenuItem>
				))}
			</DropdownMenuContent>
		</DropdownMenu>
	);
}
