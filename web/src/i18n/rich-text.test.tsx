/* Copied from localman-next apps/web/src/i18n/rich-text.test.tsx. */
import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { renderRich } from "./rich-text";

describe("renderRich", () => {
	it("renders <code> and <b> by default and keeps the text around them", () => {
		const { container } = render(
			<p>{renderRich("Run <code>tunelo http 3000</code>, <b>then</b> go.")}</p>,
		);
		expect(container.innerHTML).toBe(
			"<p>Run <code>tunelo http 3000</code>, <b>then</b> go.</p>",
		);
	});

	it("uses the renderer passed for a tag", () => {
		const { container } = render(
			<p>
				{renderRich("<code>Mock</code> REST", {
					code: (chunk) => <span className="mono">{chunk}</span>,
				})}
			</p>,
		);
		expect(container.innerHTML).toBe(
			'<p><span class="mono">Mock</span> REST</p>',
		);
	});

	it("keeps only the text of an unknown tag and escapes everything", () => {
		const { container } = render(
			<p>{renderRich("<i>x</i> & <script>alert(1)</script>")}</p>,
		);
		expect(container.querySelector("script")).toBeNull();
		expect(container.textContent).toBe("x & alert(1)");
	});
});
