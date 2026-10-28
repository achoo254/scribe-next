/**
 * Languages of the Scribe UI: the languages of the shared signed-out shell
 * (@achoo254/app-shell), so the footer switch and Scribe's own texts always
 * offer the same choices. Vietnamese is the source of every key and the
 * only language of the signed-in app; English covers the signed-out pages
 * (`messages/en/dictionary.ts`).
 */
import { SHELL_LOCALES, type ShellLocale } from "@achoo254/app-shell";

export type Locale = ShellLocale;

export const DEFAULT_LOCALE: Locale = "vi";

/** Every language of the UI, in the order the language switch lists them. */
export const LOCALES: readonly Locale[] = SHELL_LOCALES;
