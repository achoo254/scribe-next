import { useAuth } from "../auth/auth-context";
import { useUsageQuery } from "../features/files/file-queries";
import { ownKeyFor } from "../features/user-keys/own-keys";
import { useT } from "../i18n/i18n-context";
import type { MessageKey } from "../i18n/message-types";
import { cn } from "../lib/cn";
import { formatStorage } from "../lib/format";
import { providerNameKey } from "./key-providers";
import { cardClassName } from "./ui/card";
import { MeterBar, type MeterTone } from "./ui/meter-bar";
import { NeedsKeyNotice } from "./user-keys-entry";

/** Bar colour: warning from 75%, error from 90%. */
export function usageTone(percent: number): MeterTone {
	if (percent >= 90) return "error";
	if (percent >= 75) return "warning";
	return "primary";
}

function percentOf(used: number, limit: number): number {
	return limit > 0 ? (used / limit) * 100 : 100;
}

interface UsageRowProps {
	label: string;
	value: string;
	meterLabel: string;
	used: number;
	limit: number;
	"data-id": string;
}

/** One line: label and numbers, a thin bar under them. */
function UsageRow({
	label,
	value,
	meterLabel,
	used,
	limit,
	"data-id": dataId,
}: UsageRowProps) {
	const percent = percentOf(used, limit);
	return (
		<div data-id={dataId} className="grid min-w-0 gap-1">
			<div className="flex items-baseline justify-between gap-2 text-xs">
				<span className="truncate text-muted">{label}</span>
				<span
					data-id={`${dataId}-value`}
					className="shrink-0 font-mono text-fg tabular-nums"
				>
					{value}
				</span>
			</div>
			<MeterBar
				role="meter"
				size="sm"
				label={meterLabel}
				value={percent}
				tone={usageTone(percent)}
			/>
		</div>
	);
}

/** Name of a provider the backend sent, or the raw code when unknown. */
function useProviderName(): (code: string) => string {
	const t = useT();
	return (code) => {
		const key = providerNameKey(code);
		return key ? t(key) : code;
	};
}

interface OwnKeyRowProps {
	label: string;
	provider: string;
	detail?: string;
	"data-id": string;
}

/** A use that runs on the user's own key, and on which provider. */
function OwnKeyRow({
	label,
	provider,
	detail,
	"data-id": dataId,
}: OwnKeyRowProps) {
	const t = useT();
	return (
		<div data-id={dataId} className="grid min-w-0 gap-1">
			<div className="flex items-baseline justify-between gap-2 text-xs">
				<span className="truncate text-muted">{label}</span>
				<span data-id={`${dataId}-value`} className="shrink-0 text-fg">
					{t("ui.usage.ownKey.value", { provider })}
				</span>
			</div>
			{detail ? <span className="text-muted text-xs">{detail}</span> : null}
		</div>
	);
}

/**
 * Usage in one compact card above every page: how long files are kept,
 * storage, and which AI uses run on which of the user's own keys. AI runs only on the
 * user's keys, so a missing one gets a hint that opens "Key riêng". Shows
 * once `/api/me/usage` answers.
 */
export function UsageCard() {
	const t = useT();
	const { user } = useAuth();
	const { data: storage } = useUsageQuery();
	const providerName = useProviderName();
	if (!storage) return null;
	const own = user?.ownKeys ?? null;
	const transcribeKey = ownKeyFor(user, "transcribe");
	const textKey = ownKeyFor(user, "text");
	const retentionDays = user?.plan?.retentionDays ?? storage.retentionDays ?? 0;
	const storageFull =
		storage.quotaBytes > 0 && storage.usageBytes >= storage.quotaBytes;
	const keyHint: MessageKey | null =
		transcribeKey === null && textKey === null
			? "keys.needed.any"
			: transcribeKey === null
				? "keys.needed.transcribe"
				: textKey === null
					? "keys.needed.text"
					: null;

	return (
		<section
			data-id="usage"
			aria-label={t("ui.usage.region")}
			className={cn(cardClassName, "grid gap-2.5 px-4 py-3")}
		>
			{retentionDays > 0 ? (
				<span className="text-muted text-xs" data-id="usage-retention">
					{t("ui.usage.retention", { count: retentionDays })}
				</span>
			) : null}
			<div className="grid gap-x-6 gap-y-2.5 sm:grid-cols-[repeat(auto-fit,minmax(10rem,1fr))]">
				<UsageRow
					data-id="usage-storage"
					label={t("ui.usage.storage")}
					meterLabel={t("ui.usage.storageMeter")}
					value={`${formatStorage(storage.usageBytes)} / ${formatStorage(storage.quotaBytes)}`}
					used={storage.usageBytes}
					limit={storage.quotaBytes}
				/>
				{transcribeKey ? (
					<OwnKeyRow
						data-id="usage-own-transcribe"
						label={t("ui.usage.ownKey.transcribe")}
						provider={providerName(transcribeKey)}
						detail={
							own && own.minutes > 0
								? t("ui.usage.ownKey.minutes", { count: own.minutes })
								: undefined
						}
					/>
				) : null}
				{textKey ? (
					<OwnKeyRow
						data-id="usage-own-text"
						label={t("ui.usage.ownKey.text")}
						provider={providerName(textKey)}
					/>
				) : null}
			</div>
			{storageFull ? (
				<p data-id="usage-full" className="m-0 text-error-text text-xs">
					{t("ui.usage.storageFull")}
				</p>
			) : null}
			{keyHint ? (
				<NeedsKeyNotice data-id="usage-needs-key" message={t(keyHint)} />
			) : null}
		</section>
	);
}
