/* Copied from localman-next apps/web/src/components/layout/user-avatar.tsx.
   Scribe change: the session only has an email (no name, no picture), so
   the avatar is always its first letter. */
import type { CurrentUser } from "../auth/auth-api";

/** First letter of the user's email. */
export function UserAvatar({ user }: { user: CurrentUser }) {
	const initial = user.email.normalize("NFD").charAt(0).toUpperCase();
	return (
		// Drawn by CSS so the initial is not part of a labelled button's
		// visible text (axe label-content-name-mismatch).
		<span
			aria-hidden="true"
			data-initial={initial}
			className="grid size-7 shrink-0 place-items-center rounded-sm bg-primary-solid font-bold font-mono text-[0.8125rem] text-on-primary before:content-[attr(data-initial)]"
		/>
	);
}
