import { IconPencil } from "@tabler/icons-react";
import { type KeyboardEvent, useEffect, useId, useRef, useState } from "react";
import { IconButton } from "../../components/ui/icon-button";
import { Input } from "../../components/ui/input";
import { useToast } from "../../components/ui/toast";
import {
	MAX_TITLE_CHARS,
	normalizeTitle,
} from "../../features/transcript-docs/transcript-docs-api";
import { useRenameTranscript } from "../../features/transcript-docs/transcript-docs-queries";
import { useT } from "../../i18n/i18n-context";
import { errorMessage } from "../../lib/api";

export interface RecordingTitleProps {
	/** Title the user set; null = the default name is in use. */
	title: string | null;
	/** Generated name ("Bản ghi trên iPhone 13:43", the file name). */
	defaultName: string;
	/** Session the title belongs to (`s:<session>` or `f:<fileId>`). */
	transcriptKey: string;
	"data-id": string;
}

/**
 * Name of a recording in "Lịch sử ghi", renamed in place: the pencil opens
 * an input; Enter or leaving the field saves, Esc cancels, an empty name
 * goes back to the default. The list shows the new name at once and goes
 * back to the old one (with a toast) when the backend refuses.
 */
export function RecordingTitle({
	title,
	defaultName,
	transcriptKey,
	"data-id": dataId,
}: RecordingTitleProps) {
	const t = useT();
	const toast = useToast();
	const rename = useRenameTranscript();
	const hintId = useId();
	const [draft, setDraft] = useState<string | null>(null);
	// Enter, Esc and the blur that follows must end the edit only once.
	const editing = useRef(false);
	const pencil = useRef<HTMLButtonElement>(null);
	const refocus = useRef(false);
	const name = title ?? defaultName;

	useEffect(() => {
		if (draft === null && refocus.current) {
			refocus.current = false;
			pencil.current?.focus();
		}
	}, [draft]);

	function start(): void {
		editing.current = true;
		setDraft(title ?? "");
	}

	function finish(save: boolean, keepFocus: boolean): void {
		if (!editing.current || draft === null) return;
		editing.current = false;
		refocus.current = keepFocus;
		setDraft(null);
		const next = normalizeTitle(draft);
		if (!save || next === title) return;
		rename.mutate(
			{ key: transcriptKey, title: next ?? "" },
			{
				onError: (cause) =>
					toast.show({
						tone: "error",
						title: t("errors.transcripts.rename"),
						description: errorMessage(cause),
					}),
			},
		);
	}

	function onKeyDown(event: KeyboardEvent<HTMLInputElement>): void {
		// A Vietnamese IME confirms its syllable with Enter: not a save.
		if (event.nativeEvent.isComposing) return;
		if (event.key === "Enter") {
			event.preventDefault();
			finish(true, true);
		} else if (event.key === "Escape") {
			event.preventDefault();
			event.stopPropagation();
			finish(false, true);
		}
	}

	if (draft !== null) {
		return (
			<div className="grid min-w-0 flex-1 gap-1">
				<Input
					data-id={`${dataId}-input`}
					aria-label={t("record.rename.field")}
					aria-describedby={hintId}
					placeholder={defaultName}
					value={draft}
					maxLength={MAX_TITLE_CHARS}
					autoFocus
					enterKeyHint="done"
					className="min-h-9 font-sans font-semibold text-base pointer-coarse:min-h-11"
					onChange={(event) => setDraft(event.target.value)}
					onKeyDown={onKeyDown}
					onBlur={() => finish(true, false)}
				/>
				<span id={hintId} className="text-muted text-xs">
					{t("record.rename.hint")}
				</span>
			</div>
		);
	}

	return (
		<span className="flex min-w-0 flex-1 items-center gap-1">
			{/* Two lines before the ellipsis: on a phone the row's actions
			    leave the name little room. */}
			<span
				className="line-clamp-2 min-w-0 break-words font-semibold"
				title={name}
			>
				{name}
			</span>
			<IconButton
				ref={pencil}
				size="sm"
				label={t("record.rename.edit", { name })}
				data-id={`${dataId}-edit`}
				className="flex-none text-icon-muted"
				icon={<IconPencil size={16} aria-hidden="true" />}
				onClick={start}
			/>
		</span>
	);
}
