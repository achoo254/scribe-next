import { useAuth } from "../../auth/auth-context";
import { Card } from "../../components/ui/card";
import { MeterBar } from "../../components/ui/meter-bar";
import { useT } from "../../i18n/i18n-context";
import { renderRich } from "../../i18n/rich-text";

function resetDay(iso: string): string {
	const date = new Date(iso);
	return Number.isNaN(date.getTime())
		? ""
		: date.toLocaleDateString("vi-VN", { day: "numeric", month: "numeric" });
}

/**
 * Minutes of transcription left this month and why transcription is
 * closed when it is. Not merged with `QuotaBar`: that one shows storage
 * bytes from `/api/me/usage`, this one the minutes on `/api/me`.
 */
export function TranscriptionQuota() {
	const t = useT();
	const { user } = useAuth();
	const quota = user?.transcription;
	if (!quota) return null;
	const left = Math.max(0, quota.limitMinutes - quota.usedMinutes);
	const reset = resetDay(quota.resetsAt);
	return (
		<Card data-id="transcription-quota" className="grid gap-1.5 p-4 sm:px-6">
			<div className="flex flex-wrap items-baseline justify-between gap-x-3 gap-y-1 text-sm">
				<span>
					{renderRich(
						t("transcript.quota.left", {
							left,
							limit: quota.limitMinutes,
						}),
					)}
				</span>
				{reset ? (
					<span className="text-muted">
						{t("transcript.quota.resets", { date: reset })}
					</span>
				) : null}
			</div>
			<MeterBar
				value={
					quota.limitMinutes > 0
						? (quota.usedMinutes / quota.limitMinutes) * 100
						: 100
				}
				label={t("transcript.quota.meter")}
			/>
			{quota.paused ? (
				<p className="m-0 text-sm text-warning-text">
					{t("transcript.quota.paused")}
				</p>
			) : null}
			{quota.capReached ? (
				<p className="m-0 text-sm text-warning-text">
					{t("transcript.quota.capReached")}
				</p>
			) : null}
		</Card>
	);
}
