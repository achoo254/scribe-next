import { describe, expect, it } from "vitest";
import { isSearchable } from "./search-queries";

describe("isSearchable", () => {
	it("needs a lone word of two characters, or several words", () => {
		expect(isSearchable("")).toBe(false);
		expect(isSearchable("  !! ")).toBe(false);
		expect(isSearchable("h")).toBe(false);
		expect(isSearchable("đ")).toBe(false);
		expect(isSearchable("ho")).toBe(true);
		expect(isSearchable("hợp đ")).toBe(true);
	});
});
