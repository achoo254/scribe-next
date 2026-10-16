import { afterEach, describe, expect, it, vi } from "vitest";
import { openExternal, safeExternalUrl } from "./external-link";

afterEach(() => vi.restoreAllMocks());

describe("external links", () => {
	it.each(["https://auth.hoandat.com/account", "http://localhost:3000/x"])(
		"accepts %s",
		(url) => expect(safeExternalUrl(url)).toBe(url),
	);

	it.each([
		"javascript:alert(1)",
		"data:text/html,x",
		"file:///etc",
		"/account",
		"",
	])("refuses %s", (url) => expect(safeExternalUrl(url)).toBeNull());

	it("opens http(s) pages in a new tab without opener", () => {
		const open = vi.spyOn(window, "open").mockReturnValue(null);
		expect(openExternal("https://auth.hoandat.com/account")).toBe(true);
		expect(open).toHaveBeenCalledWith(
			"https://auth.hoandat.com/account",
			"_blank",
			"noopener,noreferrer",
		);
		expect(openExternal("javascript:alert(1)")).toBe(false);
		expect(open).toHaveBeenCalledTimes(1);
	});
});
