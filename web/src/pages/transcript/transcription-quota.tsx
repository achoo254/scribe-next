import { useAuth } from "../../auth/auth-context";
import { MeterBar } from "../../components/ui/meter-bar";

function resetDay(iso: string): string {
	const date = new Date(iso);
	return Number.isNaN(date.getTime())
		? ""
		: date.toLocaleDateString("vi-VN", { day: "numeric", month: "numeric" });
}

/** Minutes left this month; why transcription is closed when it is. */
export function TranscriptionQuota() {
	const { user } = useAuth();
	const quota = user?.transcription;
	if (!quota) return null;
	const left = Math.max(0, quota.limitMinutes - quota.usedMinutes);
	const reset = resetDay(quota.resetsAt);
	return (
		<div data-id="transcription-quota" className="grid gap-1.5">
			<div className="flex flex-wrap items-baseline justify-between gap-x-3 gap-y-1 text-sm">
				<span>
					Còn <strong>{left}</strong>/{quota.limitMinutes} phút chép lời tháng
					này
				</span>
				{reset ? (
					<span className="text-muted">đặt lại ngày {reset}</span>
				) : null}
			</div>
			<MeterBar
				value={
					quota.limitMinutes > 0
						? (quota.usedMinutes / quota.limitMinutes) * 100
						: 100
				}
				label="Phút chép lời đã dùng"
			/>
			{quota.paused ? (
				<p className="m-0 text-sm text-warning-text">
					Chép lời đang tạm ngưng, quản trị đã được báo. Thử lại sau.
				</p>
			) : null}
			{quota.capReached ? (
				<p className="m-0 text-sm text-warning-text">
					Chép lời đã dùng hết hạn mức chung của tháng này, sẽ mở lại đầu tháng
					sau.
				</p>
			) : null}
		</div>
	);
}
