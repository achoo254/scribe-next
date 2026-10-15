import type { ManagedFile, RecordingItem } from "./files-api";

const DAY_MS = 86_400_000;

export interface DayGroup {
	key: string;
	label: string;
	/** Local midnight of the day; the archive range is [from, to). */
	from: number;
	to: number;
	items: RecordingItem[];
}

/**
 * Groups recordings by local day. The backend sorts newest first, so the
 * groups come out in that order too. A missing date falls into 1970.
 */
export function groupByDay(items: readonly RecordingItem[]): DayGroup[] {
	const groups = new Map<string, DayGroup>();
	for (const item of items) {
		const parsed = item.createdAt ? new Date(item.createdAt) : null;
		const date =
			parsed && !Number.isNaN(parsed.getTime()) ? parsed : new Date(0);
		const y = date.getFullYear();
		const m = date.getMonth();
		const d = date.getDate();
		const key = `${y}-${m}-${d}`;
		let group = groups.get(key);
		if (!group) {
			const from = new Date(y, m, d).getTime();
			group = {
				key,
				label: date.toLocaleDateString("vi-VN", {
					weekday: "long",
					day: "2-digit",
					month: "2-digit",
					year: "numeric",
				}),
				from,
				// Next local midnight (not +24h: DST days are 23 or 25 hours).
				to: new Date(y, m, d + 1).getTime(),
				items: [],
			};
			groups.set(key, group);
		}
		group.items.push(item);
	}
	return [...groups.values()];
}

/** Whole days left before the backend sweeps the file; null when unknown. */
export function daysUntilExpiry(
	createdAt: string | undefined,
	retentionDays: number,
	now: number = Date.now(),
): number | null {
	if (!createdAt || retentionDays <= 0) return null;
	const created = new Date(createdAt).getTime();
	if (Number.isNaN(created)) return null;
	return Math.ceil((created + retentionDays * DAY_MS - now) / DAY_MS);
}

/** "tự xoá sau 3 ngày"; empty when the retention is unknown. */
export function expiryLabel(days: number | null): string {
	if (days === null) return "";
	if (days <= 0) return "sắp tự xoá";
	return `tự xoá sau ${days} ngày`;
}

/** Three days or less: highlighted so the user downloads in time. */
export function expiresSoon(days: number | null): boolean {
	return days !== null && days <= 3;
}

export interface OutputGroup {
	jobId: string;
	count: number;
	bytes: number;
	createdAt?: string;
	ids: string[];
}

/** Split segments grouped by the job that produced them. */
export function groupOutputsByJob(
	files: readonly ManagedFile[],
): OutputGroup[] {
	const groups = new Map<string, OutputGroup>();
	for (const file of files) {
		if (file.kind !== "output") continue;
		const key = file.jobId ?? "—";
		let group = groups.get(key);
		if (!group) {
			group = {
				jobId: key,
				count: 0,
				bytes: 0,
				createdAt: file.createdAt,
				ids: [],
			};
			groups.set(key, group);
		}
		group.count += 1;
		group.bytes += file.size;
		group.ids.push(file.id);
	}
	return [...groups.values()];
}
