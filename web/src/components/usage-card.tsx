import type { PlanInfo } from "../auth/auth-api";
import { useAuth } from "../auth/auth-context";
import { useUsageQuery } from "../features/files/file-queries";
import { useT } from "../i18n/i18n-context";
import type { MessageKey } from "../i18n/message-types";
import { cn } from "../lib/cn";
import { formatDate, formatDayMonth, formatStorage } from "../lib/format";
import { useTenantInfo } from "../lib/tenant-info";
import { PROVIDER_INFO } from "./key-providers";
import { Badge } from "./ui/badge";
import { cardClassName } from "./ui/card";
import { MeterBar, type MeterTone } from "./ui/meter-bar";

/** Label of each plan ("Gói Free"), shared with the account menu. */
export const PLAN_LABEL = {
	free: "ui.usage.plan.free",
	plus: "ui.usage.plan.plus",
} as const satisfies Record<PlanInfo["name"], MessageKey>;

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
		const info = Object.entries(PROVIDER_INFO).find(([key]) => key === code);
		return info ? t(info[1].name) : code;
	};
}

interface KeyRowProps {
	label: string;
	/** Whose key ("Dùng key của bạn (OpenAI)", "Dùng key AI của acme.vn"). */
	value: string;
	detail?: string;
	"data-id": string;
}

/** A use that runs on someone's own key (the user's, the domain owner's): no Scribe allowance to show. */
function KeyRow({ label, value, detail, "data-id": dataId }: KeyRowProps) {
	return (
		<div data-id={dataId} className="grid min-w-0 gap-1">
			<div className="flex items-baseline justify-between gap-2 text-xs">
				<span className="truncate text-muted">{label}</span>
				<span data-id={`${dataId}-value`} className="shrink-0 text-fg">
					{value}
				</span>
			</div>
			{detail ? <span className="text-muted text-xs">{detail}</span> : null}
		</div>
	);
}

/**
 * The plan and what of it is used, in one compact card above every page:
 * plan badge (with its end date) and how long files are kept, then storage,
 * this month's transcription minutes and, on the free plan, today's. Shows
 * once `/api/me/usage` answers; the minutes only for accounts that may
 * transcribe. On a tenant's domain the runs use the owner's AI key
 * (`transcription.keySource: "tenant"`): no minutes, one line saying whose
 * key; the storage of the plan still applies.
 */
export function UsageCard() {
	const t = useT();
	const { user } = useAuth();
	const { data: storage } = useUsageQuery();
	const providerName = useProviderName();
	const tenant = useTenantInfo();
	if (!storage) return null;
	const own = user?.ownKeys ?? null;
	const tenantKey = user?.transcription?.keySource === "tenant";
	const plan: PlanInfo["name"] = user?.plan?.name ?? storage.plan ?? "free";
	const until = user?.plan?.until ?? null;
	const retentionDays = user?.plan?.retentionDays ?? storage.retentionDays ?? 0;
	// Transcription on the user's own key or the domain owner's: Scribe's minutes do not apply.
	const minutes =
		own?.transcribe || tenantKey ? null : (user?.transcription ?? null);
	const reset = formatDayMonth(minutes?.resetsAt);
	const storageFull =
		storage.quotaBytes > 0 && storage.usageBytes >= storage.quotaBytes;

	return (
		<section
			data-id="usage"
			aria-label={t("ui.usage.region")}
			className={cn(cardClassName, "grid gap-2.5 px-4 py-3")}
		>
			<div className="flex flex-wrap items-center justify-between gap-x-3 gap-y-1">
				<div className="flex min-w-0 items-center gap-2">
					<Badge
						tone={plan === "plus" ? "primary" : "neutral"}
						data-id="usage-plan"
					>
						{t(PLAN_LABEL[plan])}
					</Badge>
					{until ? (
						<span className="text-muted text-xs">
							{t("ui.usage.planUntil", { date: formatDate(until) })}
						</span>
					) : null}
				</div>
				<span className="text-muted text-xs" data-id="usage-retention">
					{retentionDays > 0
						? t("ui.usage.retention", { count: retentionDays })
						: null}
					{retentionDays > 0 && minutes && reset ? " · " : null}
					{minutes && reset ? t("ui.usage.resets", { date: reset }) : null}
				</span>
			</div>
			<div className="grid gap-x-6 gap-y-2.5 sm:grid-cols-[repeat(auto-fit,minmax(10rem,1fr))]">
				<UsageRow
					data-id="usage-storage"
					label={t("ui.usage.storage")}
					meterLabel={t("ui.usage.storageMeter")}
					value={`${formatStorage(storage.usageBytes)} / ${formatStorage(storage.quotaBytes)}`}
					used={storage.usageBytes}
					limit={storage.quotaBytes}
				/>
				{minutes ? (
					<UsageRow
						data-id="usage-month"
						label={t("ui.usage.month")}
						meterLabel={t("ui.usage.monthMeter")}
						value={t("ui.usage.minutes", {
							used: minutes.usedMinutes,
							limit: minutes.limitMinutes,
						})}
						used={minutes.usedMinutes}
						limit={minutes.limitMinutes}
					/>
				) : null}
				{tenantKey && !(own?.transcribe && own?.text) ? (
					<KeyRow
						data-id="usage-tenant-key"
						label={t("ui.usage.tenantKey.label")}
						value={t("ui.usage.tenantKey.value", { domain: tenant.domain })}
					/>
				) : null}
				{own?.transcribe ? (
					<KeyRow
						data-id="usage-own-transcribe"
						label={t("ui.usage.ownKey.transcribe")}
						value={t("ui.usage.ownKey.value", {
							provider: providerName(own.transcribe),
						})}
						detail={
							own.minutes > 0
								? t("ui.usage.ownKey.minutes", { count: own.minutes })
								: undefined
						}
					/>
				) : null}
				{own?.text ? (
					<KeyRow
						data-id="usage-own-text"
						label={t("ui.usage.ownKey.text")}
						value={t("ui.usage.ownKey.value", {
							provider: providerName(own.text),
						})}
					/>
				) : null}
				{minutes?.daily ? (
					<UsageRow
						data-id="usage-today"
						label={t("ui.usage.today")}
						meterLabel={t("ui.usage.todayMeter")}
						value={t("ui.usage.minutes", {
							used: minutes.daily.usedMinutes,
							limit: minutes.daily.limitMinutes,
						})}
						used={minutes.daily.usedMinutes}
						limit={minutes.daily.limitMinutes}
					/>
				) : null}
			</div>
			{storageFull ? (
				<p data-id="usage-full" className="m-0 text-error-text text-xs">
					{t("ui.usage.storageFull")}
				</p>
			) : null}
			{minutes?.paused ? (
				<p className="m-0 text-warning-text text-xs">
					{t("transcript.quota.paused")}
				</p>
			) : null}
			{minutes?.capReached ? (
				<p className="m-0 text-warning-text text-xs">
					{t("transcript.quota.capReached")}
				</p>
			) : null}
		</section>
	);
}
