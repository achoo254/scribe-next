import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { TestProviders } from "../test/test-providers";
import { EcosystemAppSwitcher } from "./ecosystem-app-switcher";

const UTM = "utm_source=scribe&utm_medium=app_switcher";

async function openPanel() {
	render(
		<TestProviders>
			<EcosystemAppSwitcher />
		</TestProviders>,
	);
	await userEvent.click(
		screen.getByRole("button", { name: "Chuyển ứng dụng" }),
	);
	return screen.getByRole("dialog", { name: "Ứng dụng" });
}

afterEach(() => {
	vi.unstubAllGlobals();
	vi.restoreAllMocks();
});

describe("EcosystemAppSwitcher", () => {
	it("lists the other apps of the primary tenant in Vietnamese, not Scribe", async () => {
		const panel = await openPanel();
		const links = within(panel).getAllByRole("link");
		expect(links.map((link) => link.getAttribute("href"))).toEqual([
			`https://chat.hoandat.com/?${UTM}`,
			`https://localman.hoandat.com/?${UTM}`,
			`https://tunnel.hoandat.com/?${UTM}`,
			`https://auth.hoandat.com/account?${UTM}`,
		]);
		expect(within(panel).getByText("Quản lý tài khoản")).toBeVisible();
	});

	it("opens a link in the system browser from the native shell", async () => {
		vi.stubGlobal("Capacitor", { isNativePlatform: () => true });
		const open = vi.spyOn(window, "open").mockReturnValue(null);
		const panel = await openPanel();
		await userEvent.click(within(panel).getAllByRole("link")[0] as Element);
		expect(open).toHaveBeenCalledWith(
			`https://chat.hoandat.com/?${UTM}`,
			"_blank",
			"noopener,noreferrer",
		);
	});
});
