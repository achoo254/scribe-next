import type { ReactNode } from "react";
import { Navigate } from "react-router";
import { BrandMark } from "../components/brand/wordmark";
import { useAuth } from "./auth-context";

/** Full-page wait while `/api/me` answers, so no route flashes. */
function SessionCheck() {
	return (
		<output
			data-id="session-check"
			className="grid min-h-dvh place-items-center text-muted text-sm"
		>
			<span className="grid justify-items-center gap-3">
				<BrandMark size={40} className="motion-safe:animate-pulse" />
				Đang kiểm tra phiên đăng nhập…
			</span>
		</output>
	);
}

/** Children only for a signed-in user; otherwise /login. */
export function RequireAuth({ children }: { children: ReactNode }) {
	const { ready, user } = useAuth();
	if (!ready) return <SessionCheck />;
	if (!user) return <Navigate to="/login" replace />;
	return children;
}

/** Children only for a visitor; a signed-in user goes to the app. */
export function RequireGuest({ children }: { children: ReactNode }) {
	const { ready, user } = useAuth();
	if (!ready) return <SessionCheck />;
	if (user) return <Navigate to="/" replace />;
	return children;
}
