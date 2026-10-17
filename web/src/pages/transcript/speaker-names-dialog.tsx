import { type FormEvent, useEffect, useState } from "react";
import { Alert } from "../../components/ui/alert";
import { Button } from "../../components/ui/button";
import { Input } from "../../components/ui/input";
import { Modal, ModalClose } from "../../components/ui/modal";
import { ModalFooter } from "../../components/ui/modal-footer";
import { speakerIds } from "../../features/transcript-docs/reader-model";
import { useT } from "../../i18n/i18n-context";
import { errorMessage } from "../../lib/api";
import { speakerName } from "./reader-segment";

/** Longest speaker name the backend accepts. */
const MAX_NAME = 40;

export interface SpeakerNamesDialogProps {
	open: boolean;
	onOpenChange: (open: boolean) => void;
	/** Current display name by speaker id. */
	names: Readonly<Record<string, string>>;
	/** Saves every name; an empty one goes back to "Người N". */
	onSave: (names: Record<string, string>) => Promise<void>;
}

/**
 * "Người nói": one field per speaker of the transcript. A rename applies
 * everywhere (reader, downloads, minutes); sentences keep their speaker.
 */
export function SpeakerNamesDialog({
	open,
	onOpenChange,
	names,
	onSave,
}: SpeakerNamesDialogProps) {
	const t = useT();
	const ids = speakerIds(names);
	const [drafts, setDrafts] = useState<Record<string, string>>({});
	const [saving, setSaving] = useState(false);
	const [error, setError] = useState("");

	// Each opening starts from the names as they are now.
	useEffect(() => {
		if (open) {
			setDrafts({ ...names });
			setError("");
		}
	}, [open, names]);

	async function submit(event: FormEvent): Promise<void> {
		event.preventDefault();
		setSaving(true);
		setError("");
		try {
			await onSave(drafts);
			onOpenChange(false);
		} catch (cause) {
			setError(errorMessage(cause));
		} finally {
			setSaving(false);
		}
	}

	return (
		<Modal
			open={open}
			onOpenChange={onOpenChange}
			title={t("transcript.speakers.title")}
			description={t("transcript.speakers.description")}
			size="sm"
			data-id="speakers-dialog"
			footer={
				<ModalFooter>
					<ModalClose asChild>
						<Button variant="ghost" disabled={saving}>
							{t("common.actions.cancel")}
						</Button>
					</ModalClose>
					<Button
						type="submit"
						form="speakers-form"
						variant="primary"
						data-id="speakers-dialog-save"
						loading={saving}
					>
						{t("transcript.speakers.save")}
					</Button>
				</ModalFooter>
			}
		>
			<form
				id="speakers-form"
				className="grid gap-3"
				onSubmit={(event) => void submit(event)}
			>
				{ids.map((id) => {
					const fallback = speakerName(t, undefined, id);
					return (
						<div key={id} className="grid gap-1">
							<label
								htmlFor={`speaker-name-${id}`}
								className="text-muted text-sm"
							>
								{fallback}
							</label>
							<Input
								id={`speaker-name-${id}`}
								data-id={`speakers-dialog-${id}`}
								aria-label={t("transcript.speakers.field", {
									name: fallback,
								})}
								placeholder={fallback}
								value={drafts[id] ?? ""}
								maxLength={MAX_NAME}
								onChange={(event) =>
									setDrafts((value) => ({
										...value,
										[id]: event.target.value,
									}))
								}
							/>
						</div>
					);
				})}
				{error ? (
					<Alert tone="danger" data-id="speakers-dialog-error">
						{error}
					</Alert>
				) : null}
			</form>
		</Modal>
	);
}
