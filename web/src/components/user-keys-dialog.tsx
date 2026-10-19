import { IconExternalLink } from "@tabler/icons-react";
import { type FormEvent, useState } from "react";
import {
	KEY_PROVIDERS,
	type KeyProvider,
	type StoredKey,
} from "../features/user-keys/user-keys-api";
import {
	useDeleteUserKey,
	useSaveUserKey,
	useUserKeysQuery,
} from "../features/user-keys/user-keys-queries";
import { useT } from "../i18n/i18n-context";
import type { MessageKey } from "../i18n/message-types";
import { errorMessage } from "../lib/api";
import { openExternal } from "../lib/external-link";
import { formatDayMonth } from "../lib/format";
import { KeyChoiceSection } from "./key-choice-section";
import { PROVIDER_INFO } from "./key-providers";
import { Alert } from "./ui/alert";
import { Button } from "./ui/button";
import { FormField } from "./ui/form-field";
import { Input } from "./ui/input";
import { LoadError } from "./ui/load-error";
import { Modal, ModalClose } from "./ui/modal";
import { ModalFooter } from "./ui/modal-footer";
import { useToast } from "./ui/toast";

const STATUS_TEXT = {
	invalid: "keys.dialog.status.invalid",
	quota: "keys.dialog.status.quota",
	unreadable: "keys.dialog.status.unreadable",
} as const satisfies Record<Exclude<StoredKey["status"], "valid">, MessageKey>;

interface ProviderKeyProps {
	provider: KeyProvider;
	stored: StoredKey | undefined;
	/** The server cannot store keys: saving is off, deleting still works. */
	saveDisabled: boolean;
}

/**
 * One provider: what its key is used for, the saved key as "••••1234 ·
 * đã kiểm 19/10", a password field to add or replace it (checked with
 * the provider before it is stored), and delete.
 */
function ProviderKey({ provider, stored, saveDisabled }: ProviderKeyProps) {
	const t = useT();
	const toast = useToast();
	const save = useSaveUserKey();
	const remove = useDeleteUserKey();
	const [value, setValue] = useState("");
	const info = PROVIDER_INFO[provider];
	const name = t(info.name);

	function submit(event: FormEvent): void {
		event.preventDefault();
		const key = value.trim();
		if (!key) return;
		save.mutate(
			{ provider, key },
			{
				onSuccess: () => {
					setValue("");
					toast.show({
						title: t("keys.dialog.savedToast", { provider: name }),
					});
				},
			},
		);
	}

	function deleteKey(): void {
		remove.mutate(provider, {
			onSuccess: () =>
				toast.show({
					title: t("keys.dialog.deletedToast", { provider: name }),
				}),
			onError: (cause) =>
				toast.show({
					tone: "error",
					title: t("errors.keys.delete"),
					description: errorMessage(cause),
				}),
		});
	}

	return (
		<section
			data-id={`user-key-${provider}`}
			aria-labelledby={`user-key-${provider}-title`}
			className="grid gap-2 rounded-md border border-border p-3"
		>
			<div className="flex flex-wrap items-baseline justify-between gap-x-3 gap-y-1">
				<h3
					id={`user-key-${provider}-title`}
					className="m-0 font-semibold text-fg text-sm"
				>
					{name}
				</h3>
				<span
					data-id={`user-key-${provider}-saved`}
					className="font-mono text-muted text-xs"
				>
					{stored
						? t("keys.dialog.saved", {
								last4: stored.last4,
								date: formatDayMonth(stored.lastCheckedAt ?? stored.updatedAt),
							})
						: t("keys.dialog.none")}
				</span>
			</div>
			<p className="m-0 text-muted text-xs">{t(info.usedFor)}</p>
			{stored && stored.status !== "valid" ? (
				<Alert tone="warning" data-id={`user-key-${provider}-status`}>
					{t(STATUS_TEXT[stored.status])}
				</Alert>
			) : null}
			<form className="grid gap-2" onSubmit={submit}>
				<FormField
					label={t("keys.dialog.field", { provider: name })}
					error={save.isError ? errorMessage(save.error) : undefined}
					announceError
				>
					<Input
						type="password"
						data-id={`user-key-${provider}-input`}
						autoComplete="off"
						spellCheck={false}
						autoCapitalize="off"
						value={value}
						disabled={saveDisabled || save.isPending}
						placeholder={
							stored
								? t("keys.dialog.placeholderReplace")
								: t("keys.dialog.placeholderNew")
						}
						onChange={(event) => setValue(event.target.value)}
					/>
				</FormField>
				<div className="flex flex-wrap items-center gap-2">
					<Button
						type="submit"
						size="sm"
						variant="primary"
						data-id={`user-key-${provider}-save`}
						loading={save.isPending}
						disabled={saveDisabled || !value.trim()}
					>
						{t("keys.dialog.save")}
					</Button>
					{stored ? (
						<Button
							type="button"
							size="sm"
							variant="ghost"
							data-id={`user-key-${provider}-delete`}
							loading={remove.isPending}
							onClick={deleteKey}
						>
							{t("keys.dialog.delete")}
						</Button>
					) : null}
					<a
						href={info.createUrl}
						target="_blank"
						rel="noopener noreferrer"
						data-id={`user-key-${provider}-create`}
						className="ml-auto inline-flex min-h-11 items-center gap-1 text-primary text-xs underline-offset-2 hover:underline"
						onClick={(event) => {
							if (openExternal(info.createUrl)) event.preventDefault();
						}}
					>
						{t("keys.dialog.create", { provider: name })}
						<span className="sr-only">{t("keys.dialog.opensBrowser")}</span>
						<IconExternalLink size={14} aria-hidden="true" />
					</a>
				</div>
			</form>
		</section>
	);
}

export interface UserKeysDialogProps {
	open: boolean;
	onOpenChange: (open: boolean) => void;
}

/** "Key riêng": the user's own API keys and which use runs on which. */
export function UserKeysDialog({ open, onOpenChange }: UserKeysDialogProps) {
	const t = useT();
	const query = useUserKeysQuery(open);
	const data = query.data;

	return (
		<Modal
			open={open}
			onOpenChange={onOpenChange}
			title={t("keys.dialog.title")}
			size="lg"
			data-id="user-keys-dialog"
			footer={
				<ModalFooter>
					<ModalClose asChild>
						<Button variant="primary" data-id="user-keys-close">
							{t("common.actions.close")}
						</Button>
					</ModalClose>
				</ModalFooter>
			}
		>
			<div className="grid gap-4 text-sm">
				<p className="m-0">{t("keys.dialog.intro")}</p>
				{query.isError ? (
					<LoadError
						data-id="user-keys-load-error"
						title={t("errors.keys.load")}
						error={query.error}
						retrying={query.isFetching}
						onRetry={() => void query.refetch()}
					/>
				) : !data ? (
					<p className="m-0 text-muted" aria-live="polite">
						{t("keys.dialog.loading")}
					</p>
				) : (
					<>
						{data.available ? null : (
							<Alert tone="warning" data-id="user-keys-unavailable">
								{t("keys.dialog.unavailable")}
							</Alert>
						)}
						{KEY_PROVIDERS.map((provider) => (
							<ProviderKey
								key={provider}
								provider={provider}
								stored={data.keys.find((k) => k.provider === provider)}
								saveDisabled={!data.available}
							/>
						))}
						{data.available ? <KeyChoiceSection keys={data} /> : null}
					</>
				)}
			</div>
		</Modal>
	);
}
