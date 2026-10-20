/*
 * Copied from chat-next/scripts/check-brand.mjs @a5be0b4; keep in sync until
 * shared package exists. Scribe changes: the banned former name is
 * "OneScribe", and index.html must take the name from %APP_NAME%.
 *
 * Finds a hard-coded product name in one web source file:
 * - "OneScribe", the product name before the rename, anywhere (any case);
 * - the configured appName as a whole string literal in TS/JS code outside
 *   tests (sentences that mention the product, such as i18n messages, are
 *   copy and stay as written);
 * - the configured appName as a word in HTML, which has %APP_NAME%.
 * Renaming the product must only need brand.config.json, pnpm brand:gen and
 * a rebuild.
 */

export interface BrandProblem {
	file: string;
	line: number;
	message: string;
}

const FORMER_NAME = /onescribe/i;
const CODE = /\.(ts|tsx|js|mjs|cjs)$/;
const HTML = /\.html$/;
const TEST_FILE = /(^|\/)(e2e|test)\/|\.(test|spec)\.tsx?$/;

function escapeRegExp(value: string): string {
	return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

/** Problems in `text`, the content of `file` (web/-relative, `/` separators). */
export function findBrandProblems(
	text: string,
	file: string,
	appName: string,
): BrandProblem[] {
	const name = escapeRegExp(appName);
	const literal = new RegExp(`(["'\`])${name}\\1`);
	const word = new RegExp(`\\b${name}\\b`);
	const checkLiteral = CODE.test(file) && !TEST_FILE.test(file);
	const checkWord = HTML.test(file);
	const problems: BrandProblem[] = [];
	text.split("\n").forEach((content, index) => {
		const line = index + 1;
		if (FORMER_NAME.test(content)) {
			problems.push({ file, line, message: 'contains "OneScribe"' });
		}
		if (checkLiteral && literal.test(content)) {
			problems.push({
				file,
				line,
				message: `hard-codes the product name "${appName}"; read it from src/lib/brand.ts`,
			});
		}
		if (checkWord && word.test(content)) {
			problems.push({
				file,
				line,
				message: `hard-codes the product name "${appName}"; use %APP_NAME%`,
			});
		}
	});
	return problems;
}
