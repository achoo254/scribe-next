import { useUsageQuery } from "../features/files/file-queries";
import { formatGigabytes } from "../lib/format";
import { MeterBar, type MeterTone } from "./ui/meter-bar";

export function quotaTone(percent: number): MeterTone {
	if (percent >= 90) return "error";
	if (percent >= 75) return "warning";
	return "primary";
}

/** Used / quota storage; hidden until `/api/me/usage` answers. */
export function QuotaBar() {
	const { data } = useUsageQuery();
	if (!data) return null;
	const { usageBytes, quotaBytes } = data;
	const percent = quotaBytes > 0 ? (usageBytes / quotaBytes) * 100 : 0;
	return (
		<div data-id="quota" className="grid gap-1">
			<div className="flex justify-between gap-3 text-muted text-xs">
				<span>Dung lượng</span>
				<span data-id="quota-value" className="font-mono tabular-nums">
					{formatGigabytes(usageBytes)} / {formatGigabytes(quotaBytes)} GB
				</span>
			</div>
			<MeterBar
				role="meter"
				size="sm"
				label="Dung lượng đã dùng"
				value={percent}
				tone={quotaTone(percent)}
			/>
		</div>
	);
}
