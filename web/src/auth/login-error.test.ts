import { describe, expect, it } from "vitest";
import { loginErrorFromCode } from "./login-error";

describe("loginErrorFromCode", () => {
	it("explains each code the backend sends back", () => {
		expect(loginErrorFromCode("unverified")).toMatch(/chưa được xác minh/);
		expect(loginErrorFromCode("forbidden")).toMatch(/chưa được phép/);
		expect(loginErrorFromCode("denied")).toMatch(/từ chối/);
	});

	it("falls back for unknown codes and is empty without one", () => {
		expect(loginErrorFromCode("something-new")).toMatch(/thất bại/);
		expect(loginErrorFromCode(null)).toBe("");
	});
});
