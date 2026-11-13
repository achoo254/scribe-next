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
import { activeTranslator } from "../i18n/active-translator";
import { apiUrl, isNativeShell } from "../lib/api";
import { HoandatAuth, NativeFiles, Secrets } from "../lib/native";
import {
	type CurrentUser,
	WEB_LOGIN_PATH,
	deleteSession,
	fetchCurrentUser,
} from "./auth-api";

interface AuthState {
	/** False until `/api/me` has answered once. */
	ready: boolean;
	user: CurrentUser | null;
}

interface AuthApi extends AuthState {
	/**
	 * hoandat-auth sign-in. The web leaves the page (the backend redirects to
	 * the IdP and back); the iOS app opens the sign-in sheet and resolves once
	 * the session exists. Throws with a Vietnamese message.
	 */
	login: () => Promise<void>;
	logout: () => Promise<void>;
	/** Re-reads `/api/me` (minutes on the own key after a transcription request). */
	refresh: () => Promise<void>;
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
		if (!isNativeShell()) {
			window.location.assign(apiUrl(WEB_LOGIN_PATH));
			return;
		}
		await HoandatAuth.signIn();
		const user = await fetchCurrentUser();
		if (!user) throw new Error(activeTranslator().t("login.noSession"));
		setState({ ready: true, user });
	}, []);

	/**
	 * Signs this device out. `endSession`: also end the backend session and,
	 * on the web, the hoandat-auth one (returns where to go for that).
	 */
	const signOutHere = useCallback(
		async (endSession: boolean): Promise<string | null> => {
			let endSessionUrl: string | null = null;
			// iOS: stop recording and uploads first, so nothing reaches the old
			// account after the cookie is gone (plan D5.2).
			if (isNativeShell()) {
				await NativeRecorderPlugin.signOut().catch(() => undefined);
				// Audio copied for playback belongs to this account.
				await NativeFiles.clear().catch(() => undefined);
				// Drops the device token from the Keychain (the server answers
				// even when the device is already gone).
				await HoandatAuth.signOut().catch(() => undefined);
				// Older builds kept the user's own OpenAI key in the Keychain.
				await Secrets.remove({ name: "openaiApiKey" }).catch(() => undefined);
			} else if (endSession) {
				endSessionUrl = await deleteSession();
			}
			// The next user must not see this user's cached lists.
			queryClient.clear();
			setState({ ready: true, user: null });
			return endSessionUrl;
		},
		[queryClient],
	);

	const logout = useCallback(async () => {
		const endSessionUrl = await signOutHere(true);
		// Web: also end the hoandat-auth session (it returns to the sign-in page).
		if (endSessionUrl) window.location.assign(endSessionUrl);
	}, [signOutHere]);

	const refresh = useCallback(async () => {
		const user = await fetchCurrentUser().catch(() => null);
		if (user) setState({ ready: true, user });
	}, []);

	const api = useMemo<AuthApi>(
		() => ({ ...state, login, logout, refresh }),
		[state, login, logout, refresh],
	);
	return <AuthContext.Provider value={api}>{children}</AuthContext.Provider>;
}
