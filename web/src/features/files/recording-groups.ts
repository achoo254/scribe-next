import { activeTranslator } from "../../i18n/active-translator";
import { formatTimeOfDay } from "../../lib/format";
import { transcriptKey } from "../transcript-docs/transcript-docs-api";
import type { ManagedFile, RecordingItem } from "./files-api";

const DAY_MS = 86_400_000;

/** What `groupByDay` needs from an item: a recordings row or a transcript summary. */
export interface DatedItem {
	createdAt?: string | null;
	durationS?: number | null;
}

export interface DayGroup<T extends DatedItem = RecordingItem> {
	key: string;
	/** "Hôm nay", "Hôm qua", else the weekday ("Thứ Sáu"). */
	label: string;
	/** "23/10/2026". */
	dateLabel: string;
	/** Local midnight of the day; the archive range is [from, to). */
	from: number;
	to: number;
	/** Sum of the items' durations, seconds. */
	durationS: number;
	items: T[];
}

function capitalize(text: string): string {
	return text.charAt(0).toLocaleUpperCase("vi-VN") + text.slice(1);
}

/** "Hôm nay" / "Hôm qua" for the two latest days, the weekday before that. */
function dayLabel(from: number, date: Date, now: number): string {
	const { t } = activeTranslator();
	const today = new Date(now);
	const todayFrom = new Date(
		today.getFullYear(),
		today.getMonth(),
		today.getDate(),
	).getTime();
	const yesterdayFrom = new Date(
		today.getFullYear(),
		today.getMonth(),
		today.getDate() - 1,
	).getTime();
	if (from === todayFrom) return t("record.groups.today");
	if (from === yesterdayFrom) return t("record.groups.yesterday");
	return capitalize(date.toLocaleDateString("vi-VN", { weekday: "long" }));
}

/**
 * Groups items by local day. The backend sorts newest first, so the
 * groups come out in that order too. A missing date falls into 1970.
 */
export function groupByDay<T extends DatedItem>(
	items: readonly T[],
	now: number = Date.now(),
): DayGroup<T>[] {
	const groups = new Map<string, DayGroup<T>>();
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
				label: dayLabel(from, date, now),
				dateLabel: date.toLocaleDateString("vi-VN", {
					day: "2-digit",
					month: "2-digit",
					year: "numeric",
				}),
				from,
				// Next local midnight (not +24h: DST days are 23 or 25 hours).
				to: new Date(y, m, d + 1).getTime(),
				durationS: 0,
				items: [],
			};
			groups.set(key, group);
		}
		group.items.push(item);
		group.durationS += item.durationS ?? 0;
	}
	return [...groups.values()];
}

/**
 * Whole days left before the backend sweeps the file; null when unknown.
 * The file's own `expiresAt` (fixed by the plan it was made under) wins;
 * older replies without it fall back to `createdAt` + the current plan's
 * `retentionDays`.
 */
export function daysUntilExpiry(
	file: { createdAt?: string; expiresAt?: string },
	retentionDays: number,
	now: number = Date.now(),
): number | null {
	const expires = file.expiresAt
		? new Date(file.expiresAt).getTime()
		: file.createdAt && retentionDays > 0
			? new Date(file.createdAt).getTime() + retentionDays * DAY_MS
			: Number.NaN;
	if (Number.isNaN(expires)) return null;
	return Math.ceil((expires - now) / DAY_MS);
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
	/** Name the user gave the session; null = the default "Bản ghi trên iPhone 09:41". */
	title: string | null;
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

/** iOS segment of a recording session; an uploaded file never is, whatever its name. */
function segmentOf(
	item: RecordingItem,
): { sessionId: string; index: number } | null {
	return item.kind === "upload" ? null : parseSegmentName(item.name);
}

/**
 * Key of the session a file belongs to (`s:<session>` for iOS segments,
 * `f:<id>` otherwise): the title and the transcript are stored per session.
 */
export function itemTranscriptKey(item: RecordingItem): string {
	const segment = segmentOf(item);
	return segment
		? transcriptKey({ session: segment.sessionId })
		: transcriptKey({ fileId: item.id });
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
		const segment = segmentOf(item);
		if (!segment) {
			rows.push({ kind: "single", item });
			continue;
		}
		let row = sessions.get(segment.sessionId);
		if (!row) {
			row = {
				kind: "session",
				sessionId: segment.sessionId,
				title: null,
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
		// Every part carries the session's title; take the first one set.
		row.title ??= item.title || null;
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
	const time = formatTimeOfDay(createdAt);
	if (!withDate) return `${iphoneSession} ${time}`;
	const day = date.toLocaleDateString("vi-VN", {
		day: "2-digit",
		month: "2-digit",
		year: "numeric",
	});
	return `${iphoneSession} ${time} ${day}`;
}
