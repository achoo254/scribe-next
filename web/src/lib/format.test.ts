import { afterEach, describe, expect, it } from "vitest";
import { setActiveTranslator } from "../i18n/active-translator";
import { VIETNAMESE } from "../i18n/i18n-context";
import { en } from "../i18n/messages/en/dictionary";
import { zhCN } from "../i18n/messages/zh-CN/dictionary";
import { createTranslator } from "../i18n/translate";
import {
	formatDate,
	formatDateTime,
	formatDayMonth,
	formatLocalDate,
	formatTimeAndDay,
	formatWeekday,
} from "./format";

afterEach(() => setActiveTranslator(VIETNAMESE));

const english = (): void => setActiveTranslator(createTranslator("en", en));
const chinese = (): void =>
	setActiveTranslator(createTranslator("zh-CN", zhCN));

/** 09:05 on Friday 23 October 2026 in Vietnam. */
const ISO = "2026-10-23T02:05:00Z";

/*
 * Dates follow the language on screen: Vietnamese day-first numbers by
 * default, English month names on an English page. Times stay 24-hour in
 * Vietnam time either way (the product's timezone).
 */
describe("format on a Vietnamese page", () => {
	it("writes dates day first", () => {
		expect(formatDate(ISO)).toBe("23/10/2026");
		expect(formatDayMonth(ISO)).toBe("23/10");
		expect(formatTimeAndDay(ISO)).toBe("09:05 23/10");
	});

	it("names the weekday in Vietnamese, capitalised", () => {
		expect(formatWeekday(new Date(2026, 9, 23))).toBe("Thứ Sáu");
		expect(formatLocalDate(new Date(2026, 9, 23))).toBe("23/10/2026");
	});
});

describe("format on an English page", () => {
	it("writes dates with the month name", () => {
		english();
		expect(formatDate(ISO)).toBe("Oct 23, 2026");
		expect(formatDayMonth(ISO)).toBe("Oct 23");
		expect(formatTimeAndDay(ISO)).toBe("09:05, Oct 23");
		expect(formatDateTime(ISO)).toMatch(/^Oct 23, 2026, \d\d:05:00$/);
	});

	it("names the weekday in English", () => {
		english();
		expect(formatWeekday(new Date(2026, 9, 23))).toBe("Friday");
		expect(formatLocalDate(new Date(2026, 9, 23))).toBe("Oct 23, 2026");
	});

	it("leaves empty and invalid dates empty", () => {
		english();
		expect(formatDate(null)).toBe("");
		expect(formatDayMonth("not a date")).toBe("");
		expect(formatTimeAndDay(undefined)).toBe("");
	});
});

describe("format on a Chinese page", () => {
	it("writes dates with 年, 月 and 日", () => {
		chinese();
		expect(formatDate(ISO)).toBe("2026年10月23日");
		expect(formatDayMonth(ISO)).toBe("10月23日");
		expect(formatTimeAndDay(ISO)).toBe("10月23日 09:05");
		expect(formatDateTime(ISO)).toMatch(/^2026年10月23日 \d\d:05:00$/);
	});

	it("names the weekday in Chinese", () => {
		chinese();
		expect(formatWeekday(new Date(2026, 9, 23))).toBe("星期五");
		expect(formatLocalDate(new Date(2026, 9, 23))).toBe("2026年10月23日");
	});
});
