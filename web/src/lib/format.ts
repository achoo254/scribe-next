/**
 * Display helpers shared by the pages. Dates and times follow the language
 * on screen (`activeTranslator`), Vietnamese by default.
 */
import { activeTranslator } from "../i18n/active-translator";
import { INTL_LOCALE, type Locale } from "../i18n/locales";
import { APP_TIME_ZONE } from "../i18n/translate";

/** BCP 47 tag of the language on screen. */
function intlLocale(locale: Locale = activeTranslator().locale): string {
	return INTL_LOCALE[locale];
}

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

/**
 * Storage amount for the usage card: "200 MB", "12.3 MB", "1.25 GB"
 * (trailing zeros dropped), or KB below one megabyte.
 */
export function formatStorage(bytes: number): string {
	const trim = (value: number, digits: number) =>
		String(Number(value.toFixed(digits)));
	if (bytes >= 1024 ** 3) return `${trim(bytes / 1024 ** 3, 2)} GB`;
	if (bytes >= 1024 ** 2) return `${trim(bytes / 1024 ** 2, 1)} MB`;
	return formatSize(bytes);
}

/** How each language writes a month next to a day: "1/11", "Nov 1", "11月1日". */
const MONTH_STYLE: Readonly<Record<Locale, "numeric" | "short" | "long">> = {
	vi: "numeric",
	en: "short",
	"zh-CN": "long",
};

/**
 * Day and month in Vietnam time, "1/11" (en "Nov 1", zh "11月1日"); empty
 * when invalid.
 */
export function formatDayMonth(iso?: string | null): string {
	if (!iso) return "";
	const date = new Date(iso);
	return Number.isNaN(date.getTime())
		? ""
		: date.toLocaleDateString(intlLocale(), {
				day: "numeric",
				month: MONTH_STYLE[activeTranslator().locale],
				timeZone: APP_TIME_ZONE,
			});
}

/**
 * Full date in Vietnam time, "31/12/2026" (en "Dec 31, 2026", zh
 * "2026年12月31日"); empty when invalid.
 */
export function formatDate(iso?: string | null): string {
	return iso ? activeTranslator().formatDate(iso) : "";
}

/** Local time of day, "09:41"; empty for a missing or invalid timestamp. */
export function formatTimeOfDay(iso?: string | null): string {
	if (!iso) return "";
	const date = new Date(iso);
	return Number.isNaN(date.getTime())
		? ""
		: date.toLocaleTimeString(intlLocale(), {
				hour: "2-digit",
				minute: "2-digit",
				hourCycle: "h23",
			});
}

/** Local date and time; empty for a missing or invalid timestamp. */
export function formatDateTime(iso?: string): string {
	if (!iso) return "";
	const date = new Date(iso);
	return Number.isNaN(date.getTime()) ? "" : formatLocalDateTime(date);
}

/** Local date of `date`, "23/10/2026" (en "Oct 23, 2026", zh "2026年10月23日"). */
export function formatLocalDate(date: Date): string {
	const month = MONTH_STYLE[activeTranslator().locale];
	return date.toLocaleDateString(
		intlLocale(),
		month === "numeric"
			? { day: "2-digit", month: "2-digit", year: "numeric" }
			: { day: "numeric", month, year: "numeric" },
	);
}

/** Local weekday of `date`, capitalised: "Thứ Sáu", "Friday", "星期五". */
export function formatWeekday(date: Date): string {
	const locale = intlLocale();
	const name = date.toLocaleDateString(locale, { weekday: "long" });
	return name.charAt(0).toLocaleUpperCase(locale) + name.slice(1);
}

/**
 * Local date and time of `date` in the language on screen:
 * "09:12:00 16/10/2026" (en "Oct 16, 2026, 09:12:00", zh
 * "2026年10月16日 09:12:00").
 */
export function formatLocalDateTime(date: Date): string {
	const month = MONTH_STYLE[activeTranslator().locale];
	if (month === "numeric") {
		return date.toLocaleString(intlLocale(), { hourCycle: "h23" });
	}
	return date.toLocaleString(intlLocale(), {
		day: "numeric",
		month,
		year: "numeric",
		hour: "2-digit",
		minute: "2-digit",
		second: "2-digit",
		hourCycle: "h23",
	});
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

/** Month of `formatTimeAndDay`: digits, except English's short name. */
const TIME_AND_DAY_MONTH: Readonly<
	Record<Locale, "2-digit" | "numeric" | "short">
> = {
	vi: "2-digit",
	en: "short",
	"zh-CN": "numeric",
};

/**
 * "00:00 17/10" (en "00:00, Oct 17", zh "10月17日 00:00") in Vietnam time:
 * when a limit opens again or a link expires. Empty for a missing or
 * invalid timestamp.
 */
export function formatTimeAndDay(iso?: string | null): string {
	if (!iso) return "";
	const date = new Date(iso);
	if (Number.isNaN(date.getTime())) return "";
	const locale = activeTranslator().locale;
	const parts = new Intl.DateTimeFormat(intlLocale(), {
		hour: "2-digit",
		minute: "2-digit",
		day: locale === "vi" ? "2-digit" : "numeric",
		month: TIME_AND_DAY_MONTH[locale],
		hourCycle: "h23",
		timeZone: APP_TIME_ZONE,
	}).formatToParts(date);
	const part = (type: Intl.DateTimeFormatPartTypes) =>
		parts.find((p) => p.type === type)?.value ?? "";
	const time = `${part("hour")}:${part("minute")}`;
	switch (locale) {
		case "en":
			return `${time}, ${part("month")} ${part("day")}`;
		case "zh-CN":
			return `${part("month")}月${part("day")}日 ${time}`;
		case "vi":
			return `${time} ${part("day")}/${part("month")}`;
	}
}
