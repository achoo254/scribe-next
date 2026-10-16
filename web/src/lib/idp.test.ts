import { describe, expect, it } from "vitest";
import { IDP_ACCOUNT_URL, IDP_APPS_URL, idpOriginOf } from "./idp";

describe("IdP links", () => {
	it("default to hoandat-auth when VITE_IDP_ORIGIN is not set", () => {
		expect(IDP_ACCOUNT_URL).toBe("https://auth.hoandat.com/account");
		expect(IDP_APPS_URL).toBe("https://auth.hoandat.com/account#account-apps");
	});

	it("keeps only the origin of an http(s) value", () => {
		expect(idpOriginOf("https://auth.example.com/")).toBe(
			"https://auth.example.com",
		);
		expect(idpOriginOf("http://127.0.0.1:4000")).toBe("http://127.0.0.1:4000");
	});

	it.each([
		"",
		"auth.hoandat.com",
		"javascript:alert(1)",
		"ftp://auth.hoandat.com",
	])("refuses %s", (value) => expect(idpOriginOf(value)).toBeNull());
});
