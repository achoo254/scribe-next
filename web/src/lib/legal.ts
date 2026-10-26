/**
 * Facts of the public legal pages that are not text: the effective date,
 * the support contact and the processors' policy links.
 */

/** Day the current privacy policy and terms took effect (ISO). */
export const LEGAL_EFFECTIVE_DATE = "2026-10-26T00:00:00+07:00";

/** Email address of a build value, or null when it is not one. */
export function supportEmailOf(value: string | undefined): string | null {
	const email = value?.trim() ?? "";
	return /^[^\s@<>]+@[^\s@<>]+\.[^\s@<>]+$/.test(email) ? email : null;
}

/** Where users write to the operator (`VITE_SUPPORT_EMAIL`); null: not set. */
export const SUPPORT_EMAIL = supportEmailOf(import.meta.env.VITE_SUPPORT_EMAIL);

/** Privacy policies of the processors, by the tag the text opens them with. */
export const PROCESSOR_POLICY_URLS = {
	cloudflare: "https://www.cloudflare.com/privacypolicy/",
	elevenlabs: "https://elevenlabs.io/privacy-policy",
	openai: "https://openai.com/policies/privacy-policy/",
	google: "https://policies.google.com/privacy",
	anthropic: "https://www.anthropic.com/legal/privacy",
} as const;
