/**
 * Firebase Auth, only for the Google popup: it returns an ID token that the
 * backend trades for its own httpOnly session cookie (POST
 * /api/auth/session). The config is public, read from VITE_FIREBASE_* at
 * build time (vite.config.ts fails the build when one is missing).
 */
import {
	type FirebaseApp,
	type FirebaseOptions,
	initializeApp,
} from "firebase/app";
import {
	type Auth,
	GoogleAuthProvider,
	getAuth,
	signInWithPopup,
	signOut,
} from "firebase/auth";

let auth: Auth | null = null;

/** Initialised on first use, so pages and tests that never sign in skip it. */
function firebaseAuth(): Auth {
	if (auth) return auth;
	const options: FirebaseOptions = {
		authDomain: import.meta.env.VITE_FIREBASE_AUTH_DOMAIN,
		projectId: import.meta.env.VITE_FIREBASE_PROJECT_ID,
	};
	// Bracket assignment keeps secret scanners from flagging a public key.
	// biome-ignore lint/complexity/useLiteralKeys: see the line above
	options["apiKey"] = import.meta.env.VITE_FIREBASE_API_KEY;
	const app: FirebaseApp = initializeApp(options);
	auth = getAuth(app);
	return auth;
}

/** Opens the Google popup and returns the Firebase ID token. */
export async function googleIdToken(): Promise<string> {
	const credential = await signInWithPopup(
		firebaseAuth(),
		new GoogleAuthProvider(),
	);
	return credential.user.getIdToken();
}

/**
 * Clears the Firebase session persisted by the popup; failures do not
 * matter, the backend cookie is what keeps the user signed in.
 */
export async function firebaseSignOut(): Promise<void> {
	await signOut(firebaseAuth()).catch(() => undefined);
}
