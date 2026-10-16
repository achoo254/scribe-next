/* Copied from localman-next apps/web/src/i18n/rich-text.tsx. */
import { Fragment, type ReactNode } from "react";

/** Renders the text between `<tag>` and `</tag>` of a message. */
export type TagRenderer = (chunk: string) => ReactNode;

export type TagRenderers = Readonly<Record<string, TagRenderer>>;

/**
 * Tags every message may use without passing a renderer: `<code>` for
 * paths, commands and field names, `<b>` for emphasis.
 */
const DEFAULT_TAGS: TagRenderers = {
	code: (chunk) => <code>{chunk}</code>,
	b: (chunk) => <b>{chunk}</b>,
};

const TAG = /<(\w+)>([^<]*)<\/\1>/g;

/**
 * Turns a translated text with simple, non-nested tags into React nodes,
 * so a sentence stays whole in the dictionary instead of being split
 * around its `<code>` parts. A tag with no renderer keeps only its text.
 */
export function renderRich(text: string, tags: TagRenderers = {}): ReactNode {
	const renderers = { ...DEFAULT_TAGS, ...tags };
	const nodes: ReactNode[] = [];
	let last = 0;
	for (const match of text.matchAll(TAG)) {
		const [whole, name = "", chunk = ""] = match;
		if (match.index > last) nodes.push(text.slice(last, match.index));
		const render = renderers[name];
		nodes.push(render ? render(chunk) : chunk);
		last = match.index + whole.length;
	}
	if (last < text.length) nodes.push(text.slice(last));
	return nodes.map((node, index) => (
		// biome-ignore lint/suspicious/noArrayIndexKey: parts of a fixed text never reorder.
		<Fragment key={index}>{node}</Fragment>
	));
}
