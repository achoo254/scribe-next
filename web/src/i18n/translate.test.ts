import { describe, expect, it } from "vitest";
import type { Messages } from "./message-types";
import { vi as viMessages } from "./messages/vi/dictionary";
import { createTranslator } from "./translate";

const vietnamese = createTranslator("vi", viMessages);

/*
 * No Scribe text has a placeholder or a plural form yet, so their runtime
 * is checked on a sample dictionary. The casts only widen the key type:
 * `t()` is typed by the real dictionary, which these keys are not part of.
 */
const SAMPLE = {
	greeting: "Xin chào {name}",
	files: { other: "{count} tệp" },
	year: "© {year} Scribe",
};
const sample = createTranslator("vi", SAMPLE as unknown as Messages).t as (
	key: string,
	params?: Readonly<Record<string, string | number>>,
) => string;

describe("createTranslator", () => {
	it("returns the text of a dotted key", () => {
		expect(vietnamese.t("common.actions.close")).toBe("Đóng");
		expect(vietnamese.t("ui.toast.close")).toBe("Đóng thông báo");
	});

	it("fills {name} placeholders and formats numbers the Vietnamese way", () => {
		expect(sample("greeting", { name: "Minh Anh" })).toBe("Xin chào Minh Anh");
		expect(sample("files", { count: 10_000 })).toBe("10.000 tệp");
	});

	it("keeps strings as given, so a year is not grouped", () => {
		expect(sample("year", { year: "2026" })).toBe("© 2026 Scribe");
	});

	it("shows the key instead of crashing on a missing one", () => {
		expect(sample("missing.key")).toBe("missing.key");
	});

	it("formats dates in Asia/Saigon as dd/mm/yyyy", () => {
		// 17:30 UTC is already the next day in Asia/Saigon.
		expect(vietnamese.formatDate("2026-10-08T17:30:00.000Z")).toBe(
			"09/10/2026",
		);
		expect(vietnamese.formatDate("not a date")).toBe("");
	});

	it("formats numbers with the Vietnamese grouping", () => {
		expect(vietnamese.formatNumber(10_000)).toBe("10.000");
	});
});

describe("t() type checks", () => {
	it("rejects unknown keys and wrong parameters at compile time", () => {
		// Only `tsc` judges these lines; at runtime they must not throw.
		const t = vietnamese.t;
		// @ts-expect-error: no such key
		t("ui.toast.missing");
		// @ts-expect-error: a message without placeholders takes no params
		t("ui.toast.close", { name: "x" });
		// @ts-expect-error: a namespace is not a message
		t("ui.toast");
		expect(t("ui.toast.region")).toBe("Thông báo");
	});

	it("requires every key in a later translation", () => {
		const { close: _dropped, ...rest } = viMessages.ui.toast;
		// @ts-expect-error: `close` is missing from the toast namespace
		const incomplete: Messages["ui"]["toast"] = rest;
		const extra: Messages["ui"]["toast"] = {
			...viMessages.ui.toast,
			// @ts-expect-error: keys the Vietnamese source does not have are refused
			unknownKey: "x",
		};
		expect(incomplete).toBeDefined();
		expect(extra).toBeDefined();
	});
});
