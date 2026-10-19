import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { type Locator, type Page, expect, test } from "@playwright/test";
import { stubApi } from "../support/api-stubs";
import { IDLE_RECORDER, stubNativeShell } from "../support/capacitor-stub";
import { E2E_ORIGIN } from "../support/e2e-origin";
import { fontsReady, preparePage } from "../support/page-helpers";
import {
	DEMO_EMAIL,
	LIVE_SESSION,
	LIVE_TRANSCRIPT,
	MEETING_PATH,
	QUESTION,
	SEARCH_QUERY,
	demoFixtures,
} from "./demo-fixtures";
import { toRgbPng } from "./png-rgb";

/*
 * Five App Store screenshots per display size, in the iOS app's UI (light
 * theme): recording with the live transcript, the reader with speakers,
 * the minutes, search and "Hỏi Scribe", the side-by-side translation.
 * Run: `pnpm run screenshots:app-store`.
 */

const OUT = path.resolve(
	path.dirname(fileURLToPath(import.meta.url)),
	"../../../docs/app-store/screenshots",
);

async function shoot(page: Page, name: string, project: string): Promise<void> {
	// No caret, no focus ring, fonts laid out.
	await page.evaluate(() => {
		if (document.activeElement instanceof HTMLElement)
			document.activeElement.blur();
	});
	await fontsReady(page);
	const shot = await page.screenshot({ animations: "disabled", caret: "hide" });
	// App Store Connect refuses PNGs with an alpha channel.
	const dir = path.join(OUT, project);
	await mkdir(dir, { recursive: true });
	await writeFile(path.join(dir, `${name}.png`), toRgbPng(shot));
}

/** Puts `target` just under the sticky top bar. */
async function scrollUnderTopBar(target: Locator): Promise<void> {
	await target.evaluate((el) => {
		const bar = document.querySelector('[data-id="topbar"]');
		const offset = (bar?.getBoundingClientRect().bottom ?? 0) + 12;
		window.scrollTo(
			0,
			el.getBoundingClientRect().top + window.scrollY - offset,
		);
	});
}

/** Where the reader's audio comes from (answered by the spec, never the server). */
const DEMO_AUDIO = "/__app-store-demo/meeting.wav";

/** Silence as long as the demo meeting (WAV, 8 kHz, 8-bit mono, ~15 MB in memory): the player shows 30:45. */
function silentWav(): Buffer {
	const samples = 1_845 * 8_000;
	const wav = Buffer.alloc(44 + samples, 0x80);
	wav.write("RIFF", 0, "latin1");
	wav.writeUInt32LE(36 + samples, 4);
	wav.write("WAVEfmt ", 8, "latin1");
	wav.writeUInt32LE(16, 16);
	wav.writeUInt16LE(1, 20);
	wav.writeUInt16LE(1, 22);
	wav.writeUInt32LE(8_000, 24);
	wav.writeUInt32LE(8_000, 28);
	wav.writeUInt16LE(1, 32);
	wav.writeUInt16LE(8, 34);
	wav.write("data", 36, "latin1");
	wav.writeUInt32LE(samples, 40);
	return wav;
}

const AUDIO = silentWav();

async function openApp(page: Page): Promise<void> {
	await preparePage(page, "light");
	await stubApi(page, demoFixtures());
	await page.route(`**${DEMO_AUDIO}`, (route) =>
		route.fulfill({ status: 200, contentType: "audio/wav", body: AUDIO }),
	);
	// After the Capacitor stub: the reader's audio comes from `NativeFiles.cache`.
	await page.addInitScript((url) => {
		const capacitor = Reflect.get(window, "Capacitor");
		const native = Reflect.get(capacitor, "nativePromise");
		Reflect.set(
			capacitor,
			"nativePromise",
			(plugin: string, method: string, options: unknown) =>
				plugin === "NativeFiles" && method === "cache"
					? Promise.resolve({ url })
					: native(plugin, method, options),
		);
	}, new URL(DEMO_AUDIO, E2E_ORIGIN).href);
	// The sticky bar is translucent with a blur the headless browser does not draw:
	// make it opaque so the page under it does not show through.
	await page.addInitScript(() => {
		document.addEventListener("DOMContentLoaded", () => {
			const style = document.createElement("style");
			style.textContent =
				'[data-id="topbar"] { background-color: var(--c-bg) !important; }';
			document.head.append(style);
		});
	});
}

test("01 recording with the live transcript", async ({ page }, info) => {
	await page.addInitScript(() => {
		window.localStorage.setItem("scribe.native.realtime", "1");
	});
	await stubNativeShell(
		page,
		DEMO_EMAIL,
		{
			...IDLE_RECORDER,
			state: "recording",
			elapsedS: 1_523,
			positionS: 1_522.6,
			sessionId: LIVE_SESSION,
		},
		LIVE_TRANSCRIPT,
	);
	await openApp(page);
	await page.goto("/record");
	await expect(page.getByTestId("native-record-timer")).toBeVisible();
	await expect(page.getByTestId("native-record")).toContainText(
		"Dạ, em cập nhật luôn ạ.",
	);
	await scrollUnderTopBar(page.getByTestId("native-record"));
	await shoot(page, "01-ghi-am", info.project.name);
});

test("02 reader with speakers", async ({ page }, info) => {
	await stubNativeShell(page, DEMO_EMAIL);
	await openApp(page);
	await page.goto(MEETING_PATH);
	await expect(
		page.getByRole("heading", {
			level: 1,
			name: "Họp kế hoạch ra mắt ứng dụng",
		}),
	).toBeVisible();
	await expect(page.getByTestId("reader-segment-0-speaker-0")).toHaveText(
		"Chị Lan",
	);
	await scrollUnderTopBar(page.getByTestId("reader-segment-0"));
	await shoot(page, "02-loi-chep", info.project.name);
});

test("03 minutes and bookmarks", async ({ page }, info) => {
	await stubNativeShell(page, DEMO_EMAIL);
	await openApp(page);
	await page.goto(MEETING_PATH);
	await expect(page.getByTestId("minutes-summary")).toBeVisible();
	await scrollUnderTopBar(page.getByTestId("minutes"));
	await shoot(page, "03-bien-ban", info.project.name);
});

test("04 search and Hỏi Scribe", async ({ page }, info) => {
	await stubNativeShell(page, DEMO_EMAIL);
	await openApp(page);
	await page.goto(`/search?q=${encodeURIComponent(SEARCH_QUERY)}`);
	await page.getByTestId("ask-input").fill(QUESTION);
	await page.getByTestId("ask-submit").click();
	await expect(page.getByTestId("ask-entry-1-answer")).toContainText(
		"bản dịch tiếng Anh",
	);
	// The answer at the bottom, the results above it.
	await page
		.getByTestId("ask")
		.evaluate((el) => el.scrollIntoView({ block: "end" }));
	await shoot(page, "04-hoi-scribe", info.project.name);
});

test("05 side-by-side translation", async ({ page }, info) => {
	await stubNativeShell(page, DEMO_EMAIL);
	await openApp(page);
	await page.goto(MEETING_PATH);
	await page.getByTestId("translate-menu").click();
	await page.getByRole("menuitemradio", { name: "Tiếng Anh" }).click();
	await expect(page.getByTestId("reader-segment-0-line-0")).toContainText(
		"Hi everyone",
	);
	await scrollUnderTopBar(page.getByTestId("translate"));
	await shoot(page, "05-ban-dich", info.project.name);
});
