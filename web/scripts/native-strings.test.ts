/*
 * The native apps' own texts (errors, notices, permission prompts, the
 * recording notification) exist in every language the web app has:
 * Vietnamese (the base), English and Simplified Chinese. iOS keeps the
 * Vietnamese default at the call site (`localized(key, vietnamese)`) and
 * translates it in en.lproj / zh-Hans.lproj; Android keeps it in
 * res/values and translates it in values-en / values-zh-rCN.
 */
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const IOS_APP = join(import.meta.dirname, "../ios/App/App");
const ANDROID_MAIN = join(import.meta.dirname, "../android/app/src/main");

function filesUnder(dir: string, extension: string): string[] {
	return readdirSync(dir).flatMap((name) => {
		const path = join(dir, name);
		if (statSync(path).isDirectory()) return filesUnder(path, extension);
		return path.endsWith(extension) ? [path] : [];
	});
}

/** Keys of an Apple `.strings` table (`"key" = "value";` lines). */
function stringsKeys(path: string): string[] {
	const text = readFileSync(path, "utf8");
	return [...text.matchAll(/^"([^"]+)"\s*=\s*"(?:[^"\\]|\\.)*";$/gm)].map(
		(m) => m[1] ?? "",
	);
}

/** Translatable `<string name="…">` entries of an Android resource file. */
function androidKeys(path: string): string[] {
	const text = readFileSync(path, "utf8");
	return [
		...text.matchAll(/<string name="([^"]+)"(?![^>]*translatable="false")/g),
	].map((m) => m[1] ?? "");
}

const sorted = (keys: Iterable<string>): string[] => [...new Set(keys)].sort();

/** Text containing Vietnamese letters (accented Latin), which must not stay hard-coded in user-facing code. */
const VIETNAMESE = /[À-ỹđĐ]/;

describe("iOS native texts", () => {
	const swift = filesUnder(IOS_APP, ".swift");
	const used = sorted(
		swift.flatMap((path) =>
			[...readFileSync(path, "utf8").matchAll(/localized\("([^"]+)"/g)].map(
				(m) => m[1] ?? "",
			),
		),
	);

	it("translates every localized key into English and Chinese, and nothing else", () => {
		expect(used.length).toBeGreaterThan(0);
		for (const lproj of ["en.lproj", "zh-Hans.lproj"]) {
			expect(
				sorted(stringsKeys(join(IOS_APP, lproj, "Localizable.strings"))),
				lproj,
			).toEqual(used);
		}
	});

	it("keeps no Vietnamese string outside localized() and the upload queue's internals", () => {
		const leftovers = swift.flatMap((path) =>
			readFileSync(path, "utf8")
				.split("\n")
				.filter((line) => !/^\s*\/\//.test(line))
				.flatMap((line) =>
					[...line.matchAll(/"((?:[^"\\]|\\.)*)"/g)].map((m) => ({
						line,
						text: m[1] ?? "",
					})),
				)
				.filter(
					({ line, text }) =>
						VIETNAMESE.test(text) && !line.includes("localized("),
				)
				.map(({ text }) => `${path.slice(IOS_APP.length + 1)}: ${text}`),
		);
		// Logged-only retry reasons, and backend messages matched to tell a lost upload.
		const internal =
			/UploadQueue\.swift: (init không|thiếu session|finalize không|danh sách bản ghi|HTTP \\\(status\) không|phiên|chưa có dữ liệu|không tồn tại)/;
		expect(leftovers.filter((l) => !internal.test(l))).toEqual([]);
	});

	it("words the permission prompts in every language", () => {
		const plist = readFileSync(join(IOS_APP, "Info.plist"), "utf8");
		const prompts = sorted(
			[...plist.matchAll(/<key>(NS\w+UsageDescription)<\/key>/g)].map(
				(m) => m[1] ?? "",
			),
		);
		expect(prompts.length).toBeGreaterThan(0);
		for (const lproj of ["en.lproj", "vi.lproj", "zh-Hans.lproj"]) {
			expect(
				sorted(stringsKeys(join(IOS_APP, lproj, "InfoPlist.strings"))),
				lproj,
			).toEqual(prompts);
		}
	});
});

describe("Android native texts", () => {
	const base = sorted(
		androidKeys(join(ANDROID_MAIN, "res/values/strings.xml")),
	);

	it("translates every text into English and Chinese, and nothing else", () => {
		for (const dir of ["values-en", "values-zh-rCN"]) {
			expect(
				sorted(androidKeys(join(ANDROID_MAIN, "res", dir, "strings.xml"))),
				dir,
			).toEqual(base);
		}
	});

	it("reads every AppText key from the resources", () => {
		const kotlin = filesUnder(join(ANDROID_MAIN, "java"), ".kt");
		const used = sorted(
			kotlin.flatMap((path) =>
				[
					...readFileSync(path, "utf8").matchAll(
						/AppText\.get\(R\.string\.(\w+)/g,
					),
				].map((m) => m[1] ?? ""),
			),
		);
		expect(used.length).toBeGreaterThan(0);
		expect(used.filter((key) => !base.includes(key))).toEqual([]);
	});

	it("keeps no Vietnamese string in Kotlin outside the upload queue's internals", () => {
		const kotlin = filesUnder(join(ANDROID_MAIN, "java"), ".kt");
		const leftovers = kotlin.flatMap((path) =>
			readFileSync(path, "utf8")
				.split("\n")
				.filter((line) => !/^\s*(\/\/|\*|\/\*\*)/.test(line))
				.flatMap((line) =>
					[...line.matchAll(/"((?:[^"\\]|\\.)*)"/g)].map((m) => m[1] ?? ""),
				)
				.filter((text) => VIETNAMESE.test(text))
				.map((text) => `${path.slice(ANDROID_MAIN.length + 1)}: ${text}`),
		);
		const internal =
			/UploadQueue\.kt: (init không|finalize không|danh sách bản ghi|HTTP \$\{reply\.status\} không|phiên|chưa có dữ liệu|không tồn tại)/;
		expect(leftovers.filter((l) => !internal.test(l))).toEqual([]);
	});
});
