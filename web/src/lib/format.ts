/** Display helpers shared by the record and split pages (vi-VN). */

const pad = (value: number): string => String(value).padStart(2, "0");

/** `m:ss`, or `h:mm:ss` from one hour; "—" when unknown. */
export function formatDuration(seconds?: number): string {
	if (seconds === undefined || !Number.isFinite(seconds) || seconds <= 0) {
		return "—";
	}
	const total = Math.round(seconds);
	const h = Math.floor(total / 3600);
	const m = Math.floor((total % 3600) / 60);
	const s = total % 60;
	return h > 0 ? `${h}:${pad(m)}:${pad(s)}` : `${m}:${pad(s)}`;
}

/** Elapsed clock of a running recording: `m:ss`, minutes keep growing. */
export function formatClock(seconds: number): string {
	const total =
		Number.isFinite(seconds) && seconds > 0 ? Math.floor(seconds) : 0;
	return `${Math.floor(total / 60)}:${pad(total % 60)}`;
}

/** "12.3 MB", or "456 KB" below one megabyte (never "0 KB"). */
export function formatSize(bytes: number): string {
	if (bytes >= 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
	return `${Math.max(1, Math.round(bytes / 1024))} KB`;
}

/** Gigabytes with two decimals, for the quota bar. */
export function formatGigabytes(bytes: number): string {
	return (bytes / 1024 / 1024 / 1024).toFixed(2);
}

/** Local date and time; empty for a missing or invalid timestamp. */
export function formatDateTime(iso?: string): string {
	if (!iso) return "";
	const date = new Date(iso);
	return Number.isNaN(date.getTime()) ? "" : date.toLocaleString("vi-VN");
}

/**
 * A number > 0 typed in an input (the backend takes fractional minutes and
 * megabytes), or a whole number ≥ 1 with `integer`; null otherwise.
 */
export function parsePositiveNumber(
	raw: string,
	{ integer = false }: { integer?: boolean } = {},
): number | null {
	if (raw.trim() === "") return null;
	const value = Number(raw);
	if (!Number.isFinite(value) || value <= 0) return null;
	if (integer && !Number.isInteger(value)) return null;
	return value;
}
