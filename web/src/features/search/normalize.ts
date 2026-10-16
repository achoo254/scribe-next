/**
 * The search normalisation of the backend (`server/scribe-be/src/search/norm.rs`),
 * done one character at a time so a match in the normalised text maps back
 * to the original text: lower case, marks removed (NFD), đ → d, anything
 * that is not a letter or digit becomes one space.
 */

const MARKS = /\p{M}/gu;
const LETTER_OR_DIGIT = /^[\p{L}\p{N}]+$/u;

/** One character normalised: letters and digits, or "" (a separator). */
function normalizeChar(char: string): string {
	const bare = char
		.normalize("NFD")
		.replace(MARKS, "")
		.replace(/[Đđ]/g, "d")
		.toLowerCase();
	return LETTER_OR_DIGIT.test(bare) ? bare : "";
}

interface Mapped {
	/** Normalised text, tokens separated by one space. */
	text: string;
	/** For each character of `text`: [start, end) of its source in the original. */
	source: Array<[number, number]>;
}

function mapText(original: string): Mapped {
	let text = "";
	const source: Array<[number, number]> = [];
	let offset = 0;
	for (const char of original) {
		const start = offset;
		offset += char.length;
		if (/^\p{M}$/u.test(char)) {
			// A mark typed separately (decomposed text) belongs to the letter before it.
			const previous = source[source.length - 1];
			if (previous && text[text.length - 1] !== " ") previous[1] = offset;
			continue;
		}
		const bare = normalizeChar(char);
		if (bare) {
			for (const piece of bare) {
				text += piece;
				source.push([start, offset]);
			}
		} else if (text && !text.endsWith(" ")) {
			text += " ";
			source.push([start, offset]);
		}
	}
	if (text.endsWith(" ")) {
		text = text.slice(0, -1);
		source.pop();
	}
	return { text, source };
}

/** The backend's normalised form of `text`. */
export function normalizeText(text: string): string {
	return mapText(text).text;
}

/** Tokens of a query, normalised. */
export function queryWords(query: string): string[] {
	return normalizeText(query).split(" ").filter(Boolean);
}

/** [start, end) in the original text. */
export type Range = [number, number];

function toRange(mapped: Mapped, from: number, to: number): Range | null {
	const first = mapped.source[from];
	const last = mapped.source[to - 1];
	return first && last ? [first[0], last[1]] : null;
}

/** Start offsets of the tokens of a normalised text. */
function tokenStarts(text: string): number[] {
	const starts: number[] = [];
	for (let i = 0; i < text.length; i++) {
		if (text[i] !== " " && (i === 0 || text[i - 1] === " ")) starts.push(i);
	}
	return starts;
}

/**
 * Parts of `text` that match `query` the way the backend matched them: the
 * words in a row (the last one as a prefix, still being typed); when the
 * sentence has no such run, each word on its own. Sorted, not overlapping.
 */
export function matchRanges(text: string, query: string): Range[] {
	const words = queryWords(query);
	if (words.length === 0) return [];
	const mapped = mapText(text);
	const starts = tokenStarts(mapped.text);
	const phrase = words.join(" ");
	const runs: Range[] = [];
	for (const start of starts) {
		if (!mapped.text.startsWith(phrase, start)) continue;
		const range = toRange(mapped, start, start + phrase.length);
		if (range) runs.push(range);
	}
	if (runs.length > 0) return runs;

	const last = words.length - 1;
	const singles: Range[] = [];
	for (const start of starts) {
		const end = mapped.text.indexOf(" ", start);
		const token = mapped.text.slice(start, end === -1 ? undefined : end);
		const hit = words.findIndex((word, index) =>
			index === last ? token.startsWith(word) : token === word,
		);
		if (hit === -1) continue;
		const length = hit === last ? (words[last]?.length ?? 0) : token.length;
		const range = toRange(mapped, start, start + length);
		if (range) singles.push(range);
	}
	return singles;
}

/** `text` cut into plain and matching parts, in order. */
export function highlightParts(
	text: string,
	query: string,
): Array<{ text: string; match: boolean }> {
	const parts: Array<{ text: string; match: boolean }> = [];
	let at = 0;
	for (const [start, end] of matchRanges(text, query)) {
		if (start > at) parts.push({ text: text.slice(at, start), match: false });
		parts.push({ text: text.slice(start, end), match: true });
		at = end;
	}
	if (at < text.length) parts.push({ text: text.slice(at), match: false });
	return parts;
}
