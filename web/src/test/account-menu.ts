import userEvent from "@testing-library/user-event";

/** The trigger (`<summary>`) of the shared account menu in the top bar. */
export function accountTrigger(): HTMLElement {
	const trigger = document.querySelector<HTMLElement>(
		'[data-id="topbar-account"]',
	);
	if (!trigger) throw new Error("no account menu in the page");
	return trigger;
}

/** Opens the account menu (a `<details>`) by its trigger; returns the menu. */
export async function openAccountMenu(
	user: ReturnType<typeof userEvent.setup> = userEvent.setup(),
): Promise<HTMLDetailsElement> {
	const trigger = accountTrigger();
	await user.click(trigger);
	const menu = trigger.closest("details");
	if (!menu?.open) throw new Error("the account menu did not open");
	return menu;
}

/** `data-id` of each item of the open menu, in order. */
export function accountMenuItems(menu: HTMLElement): (string | null)[] {
	return [...menu.querySelectorAll(".shell-account-item")].map((item) =>
		item.getAttribute("data-id"),
	);
}
