import { describe, expect, it } from "vitest";
import { micPermissionHint, startErrorMessage } from "./recorder-messages";

const MIC_ONLY = { mic: true, systemAudio: false, screen: false };
const SAFARI_IOS = {
	userAgent:
		"Mozilla/5.0 (iPhone; CPU iPhone OS 17_5 like Mac OS X) AppleWebKit/605.1.15 Version/17.5 Mobile/15E148 Safari/604.1",
	maxTouchPoints: 5,
};

describe("startErrorMessage", () => {
	it("points iOS Safari users to the aA menu when the mic is denied", () => {
		const message = startErrorMessage(
			new DOMException("denied", "NotAllowedError"),
			MIC_ONLY,
			SAFARI_IOS,
		);
		expect(message).toMatch(/^Trình duyệt chưa được cấp quyền Micro\. /);
		expect(message).toContain("“aA”");
	});

	it("asks to pick a share source when screen capture was refused", () => {
		expect(
			startErrorMessage(new DOMException("x", "NotAllowedError"), {
				...MIC_ONLY,
				screen: true,
			}),
		).toMatch(/chia sẻ màn hình/);
	});

	it.each([
		["NotFoundError", "Không tìm thấy thiết bị micro phù hợp."],
		["NotReadableError", /đang được ứng dụng khác sử dụng/],
		["SecurityError", "Ghi âm cần kết nối bảo mật (HTTPS)."],
	])("maps %s", (name, expected) => {
		expect(startErrorMessage(new DOMException("x", name), MIC_ONLY)).toMatch(
			expected,
		);
	});

	it("keeps the message of other errors (backend, network)", () => {
		expect(startErrorMessage(new Error("Vượt hạn mức"), MIC_ONLY)).toBe(
			"Vượt hạn mức",
		);
	});
});

describe("micPermissionHint", () => {
	it("knows Chrome on iOS and desktop browsers", () => {
		expect(
			micPermissionHint({
				userAgent: "Mozilla/5.0 (iPhone) CriOS/126.0",
				maxTouchPoints: 5,
			}),
		).toContain("Cài đặt iOS → Chrome");
		expect(
			micPermissionHint({
				userAgent: "Mozilla/5.0 (Windows NT 10.0) Chrome/126",
				maxTouchPoints: 0,
			}),
		).toContain("biểu tượng khoá");
	});

	it("treats an iPad reporting as Macintosh with touch as iOS", () => {
		expect(
			micPermissionHint({
				userAgent: "Mozilla/5.0 (Macintosh) Safari",
				maxTouchPoints: 5,
			}),
		).toContain("“aA”");
	});
});
