import { useQueryClient } from "@tanstack/react-query";
import {
	type ReactNode,
	createContext,
	useCallback,
	useContext,
	useEffect,
	useMemo,
	useState,
} from "react";
import { NativeRecorderPlugin } from "../features/recorder/native-recorder";
import { isNativeShell } from "../lib/api";
import { firebaseSignOut, googleIdToken } from "../lib/firebase";
import {
	type CurrentUser,
	createSession,
	deleteSession,
	fetchCurrentUser,
} from "./auth-api";

interface AuthState {
	/** False until `/api/me` has answered once. */
	ready: boolean;
	user: CurrentUser | null;
}

interface AuthApi extends AuthState {
	/** Google popup → backend session; throws with a Vietnamese message. */
	login: () => Promise<void>;
	logout: () => Promise<void>;
}

const AuthContext = createContext<AuthApi | null>(null);

export function useAuth(): AuthApi {
	const api = useContext(AuthContext);
	if (!api) throw new Error("useAuth must be used inside <AuthProvider>");
	return api;
}

/** Reads the session once (`/api/me`) and owns login/logout. */
export function AuthProvider({ children }: { children: ReactNode }) {
	const queryClient = useQueryClient();
	const [state, setState] = useState<AuthState>({ ready: false, user: null });

	useEffect(() => {
		let cancelled = false;
		fetchCurrentUser()
			.catch(() => null)
			.then((user) => {
				if (!cancelled) setState({ ready: true, user });
			});
		return () => {
			cancelled = true;
		};
	}, []);

	const login = useCallback(async () => {
		const idToken = await googleIdToken();
		try {
			const user = await createSession(idToken);
			setState({ ready: true, user });
		} catch (error) {
			// The backend refused (allowlist, bad token): drop the Firebase
			// session too, so the next attempt starts clean.
			await firebaseSignOut();
			throw error;
		}
	}, []);

	const logout = useCallback(async () => {
		// iOS: stop recording and uploads first, so nothing reaches the old
		// account after the cookie is gone (plan D5.2).
		if (isNativeShell())
			await NativeRecorderPlugin.signOut().catch(() => undefined);
		await deleteSession();
		await firebaseSignOut();
		// The next user must not see this user's cached lists.
		queryClient.clear();
		setState({ ready: true, user: null });
	}, [queryClient]);

	const api = useMemo<AuthApi>(
		() => ({ ...state, login, logout }),
		[state, login, logout],
	);
	return <AuthContext.Provider value={api}>{children}</AuthContext.Provider>;
}
