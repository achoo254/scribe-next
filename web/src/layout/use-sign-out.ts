import { useCallback, useState } from "react";
import { useNavigate } from "react-router";
import { useAuth } from "../auth/auth-context";

export interface SignOut {
	busy: boolean;
	/**
	 * Runs `beforeSignOut` (e.g. save the browser recording), then the
	 * existing logout of auth-context: hoandat-auth end-session on the web,
	 * the `HoandatAuth` plugin in the iOS app. Lands on /login.
	 */
	signOut: (beforeSignOut?: () => Promise<void>) => Promise<void>;
}

/** Sign-out of the account menu; replaces the top bar's old button. */
export function useSignOut(): SignOut {
	const { logout } = useAuth();
	const navigate = useNavigate();
	const [busy, setBusy] = useState(false);

	const signOut = useCallback(
		async (beforeSignOut?: () => Promise<void>) => {
			setBusy(true);
			try {
				await beforeSignOut?.();
				await logout();
				navigate("/login", { replace: true });
			} finally {
				setBusy(false);
			}
		},
		[logout, navigate],
	);

	return { busy, signOut };
}
