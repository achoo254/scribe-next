import type { ReactNode } from "react";
import { Navigate } from "react-router";
import { BrandMark } from "../components/brand/wordmark";
import { useT } from "../i18n/i18n-context";
import { useAuth } from "./auth-context";

/** Full-page wait while `/api/me` (or `/api/idp`) answers, so no route flashes. */
export function SessionCheck() {
	const t = useT();
	return (
		<output
			data-id="session-check"
			className="grid min-h-dvh place-items-center text-muted text-sm"
		>
			<span className="grid justify-items-center gap-3">
				<BrandMark size={40} className="motion-safe:animate-pulse" />
				{t("login.sessionCheck")}
			</span>
		</output>
	);
}

/** Children only for a signed-in user; otherwise `guest`, or /login. */
export function RequireAuth({
	children,
	guest,
}: {
	children: ReactNode;
	/** What a visitor sees instead (public pages); /login when absent. */
	guest?: ReactNode;
}) {
	const { ready, user } = useAuth();
	if (!ready) return <SessionCheck />;
	if (!user) return guest ?? <Navigate to="/login" replace />;
	return children;
}

/** Children only for a visitor; a signed-in user goes to the app. */
export function RequireGuest({ children }: { children: ReactNode }) {
	const { ready, user } = useAuth();
	if (!ready) return <SessionCheck />;
	if (user) return <Navigate to="/" replace />;
	return children;
}
