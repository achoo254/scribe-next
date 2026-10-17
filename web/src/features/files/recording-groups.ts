import { activeTranslator } from "../../i18n/active-translator";
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
	const { t } = activeTranslator();
	if (days <= 0) return t("record.groups.expiresSoon");
	return t("record.groups.expiresIn", { days });
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

/** Name of a segment recorded by the iOS app: `<sessionId>#<index>`. */
const SEGMENT_NAME = /^([0-9a-z-]{8,64})#(\d{1,9})$/;

export interface SessionRow {
	kind: "session";
	sessionId: string;
	/** Segments of one iOS recording session, in recording order. */
	parts: Array<{ index: number; item: RecordingItem }>;
	durationS: number;
	size: number;
	/** Earliest createdAt among the parts (the session start). */
	createdAt?: string;
}

export interface SingleRow {
	kind: "single";
	item: RecordingItem;
}

export type HistoryRow = SessionRow | SingleRow;

/** `{sessionId, index}` of an iOS segment name; null for other recordings. */
export function parseSegmentName(
	name: string,
): { sessionId: string; index: number } | null {
	const match = SEGMENT_NAME.exec(name);
	if (!match?.[1] || match[2] === undefined) return null;
	return { sessionId: match[1], index: Number(match[2]) };
}

/**
 * Collapses the segments of one iOS session into one row (a two-hour
 * meeting is 120 one-minute segments). Other recordings and uploaded files
 * stay single rows.
 * The row takes the position of the session's newest part, so the
 * newest-first order of the list is kept.
 */
export function groupSessions(items: readonly RecordingItem[]): HistoryRow[] {
	const rows: HistoryRow[] = [];
	const sessions = new Map<string, SessionRow>();
	for (const item of items) {
		// An uploaded file keeps its own row whatever its name.
		const segment = item.kind === "upload" ? null : parseSegmentName(item.name);
		if (!segment) {
			rows.push({ kind: "single", item });
			continue;
		}
		let row = sessions.get(segment.sessionId);
		if (!row) {
			row = {
				kind: "session",
				sessionId: segment.sessionId,
				parts: [],
				durationS: 0,
				size: 0,
			};
			sessions.set(segment.sessionId, row);
			rows.push(row);
		}
		row.parts.push({ index: segment.index, item });
		row.durationS += item.durationS ?? 0;
		row.size += item.size;
		if (item.createdAt && (!row.createdAt || item.createdAt < row.createdAt)) {
			row.createdAt = item.createdAt;
		}
	}
	for (const row of sessions.values())
		row.parts.sort((a, b) => a.index - b.index);
	return rows;
}

/**
 * Default name of an iPhone session: "Bản ghi trên iPhone 09:41", with the
 * date too (`withDate`) where sessions of different days sit together, as in
 * the transcript list.
 */
export function sessionTitle(
	createdAt?: string | null,
	{ withDate = false }: { withDate?: boolean } = {},
): string {
	const iphoneSession = activeTranslator().t("record.groups.iphoneSession");
	if (!createdAt) return iphoneSession;
	const date = new Date(createdAt);
	if (Number.isNaN(date.getTime())) return iphoneSession;
	const time = date.toLocaleTimeString("vi-VN", {
		hour: "2-digit",
		minute: "2-digit",
	});
	if (!withDate) return `${iphoneSession} ${time}`;
	const day = date.toLocaleDateString("vi-VN", {
		day: "2-digit",
		month: "2-digit",
		year: "numeric",
	});
	return `${iphoneSession} ${time} ${day}`;
}
