import ts from "typescript";
import { describe, expect, it } from "vitest";

/*
 * Every Vietnamese text the user reads lives in `messages/vi`. This scans
 * the app source for string literals, template parts and JSX text with a
 * Vietnamese letter, so a hard-coded string fails here instead of being
 * found by eye. Comments are not tokens of the syntax tree, so they never
 * count. Text the backend sends is shown as-is and never appears in the
 * source, so it needs no exception.
 */

/** Letters only Vietnamese uses here: vowels with a tone or hat mark, and đ. */
const VIETNAMESE =
	/[àáạảãâầấậẩẫăằắặẳẵèéẹẻẽêềếệểễìíịỉĩòóọỏõôồốộổỗơờớợởỡùúụủũưừứựửữỳýỵỷỹđ]/;

/**
 * Literals allowed outside the dictionary. Each entry names the file, the
 * exact text and why it is not user-facing copy. Keep it short.
 */
const ALLOWED: ReadonlyArray<{ file: string; text: string; reason: string }> =
	[];

/** App source without the dictionary, tests, test helpers and dev-only pages. */
const SOURCES: Record<string, string> = import.meta.glob(
	[
		"../**/*.{ts,tsx}",
		"!./messages/**",
		"!../**/*.test.{ts,tsx}",
		"!../test/**",
		"!../pages/dev/**",
	],
	{ query: "?raw", import: "default", eager: true },
);

interface Finding {
	file: string;
	line: number;
	text: string;
}

/** Text of a node that the user could read, or null for any other node. */
function literalText(node: ts.Node): string | null {
	if (
		ts.isStringLiteral(node) ||
		ts.isNoSubstitutionTemplateLiteral(node) ||
		ts.isTemplateHead(node) ||
		ts.isTemplateMiddle(node) ||
		ts.isTemplateTail(node)
	) {
		return node.text;
	}
	if (ts.isJsxText(node)) return node.text.trim();
	return null;
}

/** Literals and JSX text of `source` containing a Vietnamese letter. */
function findVietnamese(file: string, source: string): Finding[] {
	const tree = ts.createSourceFile(
		file,
		source,
		ts.ScriptTarget.Latest,
		true,
		file.endsWith(".tsx") ? ts.ScriptKind.TSX : ts.ScriptKind.TS,
	);
	const found: Finding[] = [];
	const visit = (node: ts.Node): void => {
		const text = literalText(node);
		if (text && VIETNAMESE.test(text.normalize("NFC").toLowerCase())) {
			const { line } = tree.getLineAndCharacterOfPosition(node.getStart());
			found.push({ file, line: line + 1, text });
		}
		ts.forEachChild(node, visit);
	};
	visit(tree);
	return found;
}

function isAllowed({ file, text }: Finding): boolean {
	return ALLOWED.some((entry) => entry.file === file && entry.text === text);
}

describe("hard-coded Vietnamese strings", () => {
	it("reads the app source", () => {
		expect(Object.keys(SOURCES).length).toBeGreaterThan(50);
		expect(
			Object.keys(SOURCES).some((file) => file.includes("messages/")),
		).toBe(false);
	});

	it("keeps every Vietnamese text in the dictionary", () => {
		const found = Object.entries(SOURCES)
			.flatMap(([file, source]) => findVietnamese(file, source))
			.filter((finding) => !isAllowed(finding))
			.map(({ file, line, text }) => `${file}:${line} ${text}`);
		expect(found).toEqual([]);
	});

	it("gives a reason for every allowed literal", () => {
		for (const entry of ALLOWED) {
			expect(entry.reason.trim(), entry.file).not.toBe("");
		}
	});

	it("finds literals, template parts and JSX text but not comments", () => {
		const source = [
			"// Ghi âm: a comment",
			"/* Đoạn: a block comment */",
			'const a = "Đã lưu";',
			"const b = `Còn ${n} phút`;",
			"const c = <p title={x}>Đang tải {y}</p>;",
			"const d = <p>{/* Bản ghi */}</p>;",
			'const e = "plain ascii";',
		].join("\n");
		expect(
			findVietnamese("sample.tsx", source).map(({ line, text }) => [
				line,
				text,
			]),
		).toEqual([
			[3, "Đã lưu"],
			[4, "Còn "],
			[4, " phút"],
			[5, "Đang tải"],
		]);
	});

	it("catches capital letters and decomposed accents", () => {
		expect(findVietnamese("a.ts", 'const a = "ĐANG GHI";')).toHaveLength(1);
		expect(findVietnamese("a.ts", `const a = "${"Ghi àm"}";`)).toHaveLength(1);
	});
});
