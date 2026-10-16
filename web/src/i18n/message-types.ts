/* Copied from localman-next apps/web/src/i18n/message-types.ts. Scribe
   change: the source of every key is the Vietnamese dictionary (the only
   language), not English. */
import type { vi } from "./messages/vi/dictionary";

/**
 * Plural message: one string per `Intl.PluralRules` category the language
 * uses. `other` is required (it is the only category of Vietnamese and
 * Chinese); `{count}` in the text is the number passed to `t()`.
 * A namespace must therefore never contain a key named `other`.
 */
export interface PluralForms {
	readonly zero?: string;
	readonly one?: string;
	readonly two?: string;
	readonly few?: string;
	readonly many?: string;
	readonly other: string;
}

/** A dictionary leaf: plain text, or the plural forms of one message. */
export type MessageLeaf = string | PluralForms;

/** Literal type of the Vietnamese dictionary, the source of every key. */
export type SourceMessages = typeof vi;

/** `T` with every text widened to `string`: the shape a translation fills. */
export type Translation<T> = T extends string
	? string
	: T extends { readonly other: string }
		? PluralForms
		: { readonly [K in keyof T]: Translation<T[K]> };

/** Every language's dictionary; a missing or extra key fails typecheck. */
export type Messages = Translation<SourceMessages>;

type Join<Prefix extends string, Key extends string> = Prefix extends ""
	? Key
	: `${Prefix}.${Key}`;

/** Dotted paths of the leaves of `T`: `"ui.toast.close"`. */
export type KeyPaths<T, Prefix extends string = ""> = {
	[K in keyof T & string]: T[K] extends MessageLeaf
		? Join<Prefix, K>
		: KeyPaths<T[K], Join<Prefix, K>>;
}[keyof T & string];

export type MessageKey = KeyPaths<SourceMessages>;

/** The source leaf at a dotted path. */
export type LeafAt<
	T,
	Path extends string,
> = Path extends `${infer Head}.${infer Rest}`
	? Head extends keyof T
		? LeafAt<T[Head], Rest>
		: never
	: Path extends keyof T
		? T[Path]
		: never;

/** Names between braces in a text: `"Hi {name}"` gives `"name"`. */
type Placeholders<Text> = Text extends `${string}{${infer Name}}${infer Rest}`
	? Name | Placeholders<Rest>
	: never;

/** A value for `{name}`; numbers are formatted for the current language. */
export type ParamValue = string | number;

type ParamsOfLeaf<Leaf> = Leaf extends string
	? [Placeholders<Leaf>] extends [never]
		? undefined
		: Record<Placeholders<Leaf>, ParamValue>
	: Leaf extends PluralForms
		? { count: number } & Record<
				Exclude<Placeholders<Leaf[keyof Leaf]>, "count">,
				ParamValue
			>
		: never;

/** Parameters `t(key, …)` needs: none, or an object naming each one. */
export type ParamsOf<K extends MessageKey> = ParamsOfLeaf<
	LeafAt<SourceMessages, K>
>;

/** Rest arguments of `t()`: omitted when the message has no placeholder. */
export type ParamsArg<K extends MessageKey> = ParamsOf<K> extends undefined
	? []
	: [params: ParamsOf<K>];
