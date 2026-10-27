import { IconPencil, IconTrash } from "@tabler/icons-react";
import { type FormEvent, useRef, useState } from "react";
import { EXPORT_FORMATS } from "../../components/transcript-download-menu";
import { Alert } from "../../components/ui/alert";
import { Button } from "../../components/ui/button";
import { ConfirmModal } from "../../components/ui/confirm-modal";
import {
	DropdownMenuItem,
	DropdownMenuLabel,
	DropdownMenuSeparator,
} from "../../components/ui/dropdown-menu";
import { Input } from "../../components/ui/input";
import { Modal, ModalClose } from "../../components/ui/modal";
import { ModalFooter } from "../../components/ui/modal-footer";
import { RowActionsMenu } from "../../components/ui/row-actions-menu";
import { useToast } from "../../components/ui/toast";
import { transcriptTitle } from "../../features/transcript-docs/reader-model";
import {
	MAX_TITLE_CHARS,
	type TranscriptSummary,
} from "../../features/transcript-docs/transcript-docs-api";
import {
	useDeleteTranscript,
	useRenameTranscript,
} from "../../features/transcript-docs/transcript-docs-queries";
import type { ExportScope } from "../../features/transcript/transcript-api";
import { useT } from "../../i18n/i18n-context";
import { errorMessage } from "../../lib/api";
import { downloadPaths, useDownload } from "../../lib/download";

/** What an export from the list covers: the whole iPhone session, or the one file. */
function listScope(key: string): ExportScope {
	if (key.startsWith("s:")) return { session: key.slice(2) };
	return { fileIds: [key.slice(2)] };
}

export interface TranscriptRowActionsProps {
	item: TranscriptSummary;
}

/**
 * ⋮ menu of a transcript row: download (original language; translations are
 * offered in the reader), rename and delete without opening the transcript.
 */
export function TranscriptRowActions({ item }: TranscriptRowActionsProps) {
	const t = useT();
	const toast = useToast();
	const download = useDownload();
	const rename = useRenameTranscript();
	const remove = useDeleteTranscript();
	const [titleDraft, setTitleDraft] = useState<string | null>(null);
	const [titleError, setTitleError] = useState("");
	const [confirmDelete, setConfirmDelete] = useState(false);
	const titleInput = useRef<HTMLInputElement>(null);
	const name = transcriptTitle(item);
	const id = `transcripts-item-${item.key}-menu`;

	async function submitTitle(event: FormEvent): Promise<void> {
		event.preventDefault();
		if (titleDraft === null) return;
		setTitleError("");
		try {
			await rename.mutateAsync({ key: item.key, title: titleDraft });
			setTitleDraft(null);
			toast.show({ title: t("transcript.reader.renamed") });
		} catch (cause) {
			setTitleError(errorMessage(cause));
		}
	}

	return (
		<>
			<RowActionsMenu
				label={t("transcript.list.actions", { name })}
				data-id={id}
			>
				{item.done > 0 ? (
					<>
						<DropdownMenuLabel>
							{t("transcript.download.menu")}
						</DropdownMenuLabel>
						{EXPORT_FORMATS.map(({ format, label, icon }) => (
							<DropdownMenuItem
								key={format}
								data-id={`${id}-${format}`}
								icon={icon}
								onSelect={() =>
									download(
										downloadPaths.transcript(listScope(item.key), format, name),
									)
								}
							>
								{label}
							</DropdownMenuItem>
						))}
						<DropdownMenuSeparator />
					</>
				) : null}
				<DropdownMenuItem
					data-id={`${id}-rename`}
					icon={<IconPencil size={16} aria-hidden="true" />}
					onSelect={() => {
						setTitleError("");
						setTitleDraft(item.title ?? "");
					}}
				>
					{t("transcript.reader.rename")}
				</DropdownMenuItem>
				<DropdownMenuItem
					danger
					data-id={`${id}-delete`}
					icon={<IconTrash size={16} aria-hidden="true" />}
					onSelect={() => setConfirmDelete(true)}
				>
					{t("transcript.reader.delete")}
				</DropdownMenuItem>
			</RowActionsMenu>

			<Modal
				open={titleDraft !== null}
				onOpenChange={(open) => {
					if (!open && !rename.isPending) setTitleDraft(null);
				}}
				title={t("transcript.reader.rename")}
				size="sm"
				initialFocusRef={titleInput}
				data-id={`${id}-rename-dialog`}
				footer={
					<ModalFooter>
						<ModalClose asChild>
							<Button variant="ghost" disabled={rename.isPending}>
								{t("common.actions.cancel")}
							</Button>
						</ModalClose>
						<Button
							type="submit"
							form={`${id}-rename-form`}
							variant="primary"
							data-id={`${id}-rename-save`}
							loading={rename.isPending}
						>
							{t("transcript.reader.saveTitle")}
						</Button>
					</ModalFooter>
				}
			>
				<form
					id={`${id}-rename-form`}
					className="grid gap-2"
					onSubmit={(event) => void submitTitle(event)}
				>
					<Input
						data-id={`${id}-rename-input`}
						aria-label={t("transcript.reader.titleField")}
						placeholder={transcriptTitle({ ...item, title: null })}
						value={titleDraft ?? ""}
						maxLength={MAX_TITLE_CHARS}
						ref={titleInput}
						onChange={(event) => setTitleDraft(event.target.value)}
					/>
					{titleError ? <Alert tone="danger">{titleError}</Alert> : null}
					<p className="m-0 text-muted text-sm">
						{t("transcript.reader.titleHint")}
					</p>
				</form>
			</Modal>

			<ConfirmModal
				open={confirmDelete}
				onOpenChange={setConfirmDelete}
				title={t("transcript.reader.deleteTitle")}
				question={t("transcript.reader.deleteQuestion", { name })}
				confirmLabel={t("common.actions.delete")}
				onConfirm={async () => {
					await remove.mutateAsync(item.key);
					toast.show({ title: t("transcript.reader.deleted", { name }) });
				}}
				data-id={`${id}-delete-confirm`}
			>
				{t("transcript.reader.deleteBody")}
			</ConfirmModal>
		</>
	);
}
