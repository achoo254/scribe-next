/**
 * Plugins of the iOS shell (web/ios/App/App/*Plugin.swift). Only called
 * when `isNativeShell()` is true; on the web these names resolve to
 * Capacitor's "not implemented" stubs and are never reached.
 */
import { registerPlugin } from "@capacitor/core";

export interface GoogleAuthPlugin {
	/** Google sign-in sheet → Firebase ID token (for POST /api/auth/session). */
	signIn(): Promise<{ idToken: string }>;
}

export interface NativeFilesPlugin {
	/**
	 * Downloads a backend `/api/...` URL with the session cookie into the
	 * app's cache and returns a URL the WebView can play (`capacitor://…`).
	 */
	cache(options: { url: string }): Promise<{ url: string }>;
	/** Downloads the URL, then opens the share sheet (Save to Files…). */
	share(options: { url: string }): Promise<void>;
}

/** Values kept in the app's Keychain (the user's own OpenAI key). */
export interface SecretsPlugin {
	get(options: { name: string }): Promise<{ value?: string }>;
	set(options: { name: string; value: string }): Promise<void>;
	remove(options: { name: string }): Promise<void>;
}

export const GoogleAuth = registerPlugin<GoogleAuthPlugin>("GoogleAuth");
export const NativeFiles = registerPlugin<NativeFilesPlugin>("NativeFiles");
export const Secrets = registerPlugin<SecretsPlugin>("Secrets");
