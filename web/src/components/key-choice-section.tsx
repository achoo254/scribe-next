import {
	type KeyChoice,
	type KeyProvider,
	type KeyUse,
	USE_PROVIDERS,
	type UserKeys,
} from "../features/user-keys/user-keys-api";
import { useSaveKeyChoice } from "../features/user-keys/user-keys-queries";
import { useT } from "../i18n/i18n-context";
import type { MessageKey } from "../i18n/message-types";
import { errorMessage } from "../lib/api";
import { PROVIDER_INFO } from "./key-providers";
import { Alert } from "./ui/alert";
import { FormField } from "./ui/form-field";
import { Select } from "./ui/select";

const USES = [
	{ use: "transcribe", label: "keys.uses.transcribe" },
	{ use: "live", label: "keys.uses.live" },
	{ use: "text", label: "keys.uses.text" },
] as const satisfies ReadonlyArray<{ use: KeyUse; label: MessageKey }>;

export interface KeyChoiceSectionProps {
	keys: UserKeys;
}

/**
 * Which provider each use runs on: a radio group per use, one option per
 * provider that can do it, enabled once that provider has a valid key. The
 * checked option is what the server actually uses (`active`); with no
 * valid key the use stays on Scribe's key. Text uses also pick a model.
 */
export function KeyChoiceSection({ keys }: KeyChoiceSectionProps) {
	const t = useT();
	const save = useSaveKeyChoice();
	const valid = new Set(
		keys.keys.filter((k) => k.status === "valid").map((k) => k.provider),
	);

	function choose(next: Partial<KeyChoice>): void {
		// The checked radio (what the server answers) shows the change.
		save.mutate({ ...keys.prefs, ...next });
	}

	const textProvider = keys.active.text;
	const models = textProvider ? (keys.models[textProvider] ?? []) : [];

	return (
		<section
			data-id="user-keys-uses"
			aria-labelledby="user-keys-uses-title"
			className="grid gap-3"
		>
			<div className="grid gap-1">
				<h3
					id="user-keys-uses-title"
					className="m-0 font-semibold text-fg text-sm"
				>
					{t("keys.uses.title")}
				</h3>
				<p className="m-0 text-muted text-xs">{t("keys.uses.hint")}</p>
			</div>
			{save.isError ? (
				<Alert tone="danger" data-id="user-keys-uses-error">
					{errorMessage(save.error)}
				</Alert>
			) : null}
			{USES.map(({ use, label }) => (
				<fieldset
					key={use}
					data-id={`user-keys-use-${use}`}
					className="m-0 grid gap-1.5 rounded-md border border-border p-3"
				>
					<legend className="px-1 font-mono font-semibold text-[0.8125rem] text-fg">
						{t(label)}
					</legend>
					<div className="flex flex-wrap gap-x-4 gap-y-1">
						{USE_PROVIDERS[use].map((provider: KeyProvider) => {
							const name = t(PROVIDER_INFO[provider].name);
							const enabled = valid.has(provider);
							return (
								<label
									key={provider}
									className="inline-flex min-h-11 cursor-pointer items-center gap-2 has-disabled:cursor-not-allowed has-disabled:text-muted"
								>
									<input
										type="radio"
										name={`user-keys-${use}`}
										data-id={`user-keys-use-${use}-${provider}`}
										className="size-4 accent-primary"
										checked={keys.active[use] === provider}
										disabled={!enabled || save.isPending}
										onChange={() => choose({ [use]: provider })}
									/>
									{enabled ? name : t("keys.uses.needsKey", { provider: name })}
								</label>
							);
						})}
					</div>
					{keys.active[use] ? null : (
						<p
							className="m-0 text-muted text-xs"
							data-id={`user-keys-use-${use}-scribe`}
						>
							{t("keys.uses.scribe")}
						</p>
					)}
					{use === "transcribe" && keys.active.transcribe === "gemini" ? (
						<p className="m-0 text-muted text-xs">
							{t("keys.uses.geminiNote")}
						</p>
					) : null}
					{use === "text" && textProvider && models.length > 1 ? (
						<FormField label={t("keys.uses.model")} className="max-w-xs">
							<Select
								data-id="user-keys-text-model"
								value={keys.active.textModel ?? models[0]?.id}
								disabled={save.isPending}
								onChange={(event) =>
									choose({ text: textProvider, textModel: event.target.value })
								}
							>
								{models.map((model) => (
									<option key={model.id} value={model.id}>
										{model.label}
									</option>
								))}
							</Select>
						</FormField>
					) : null}
				</fieldset>
			))}
		</section>
	);
}
