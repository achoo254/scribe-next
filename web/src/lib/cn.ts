/** Joins class names, skipping falsy entries (conditional classes). */
export function cn(
	...classes: ReadonlyArray<string | false | null | undefined>
): string {
	return classes.filter(Boolean).join(" ");
}
