/**
 * Plugins of the iOS shell (web/ios/App/App/*Plugin.swift). Only called
 * when `isNativeShell()` is true; on the web these names resolve to
 * Capacitor's "not implemented" stubs and are never reached.
 */
import { registerPlugin } from "@capacitor/core";

export interface HoandatAuthPlugin {
	/**
	 * hoandat-auth sign-in sheet (Code + PKCE made in the app); the backend
	 * redeems the code and sets the session cookie. Resolves with the email.
	 */
	signIn(): Promise<{ email: string }>;
	/** Ends the backend session and forgets this device. */
	signOut(): Promise<void>;
}

export interface NativeFilesPlugin {
	/**
	 * Downloads a backend `/api/...` URL with the session cookie into the
	 * app's cache and returns a URL the WebView can play (`capacitor://…`).
	 */
	cache(options: { url: string }): Promise<{ url: string }>;
	/** Downloads the URL, then opens the share sheet (Save to Files…). */
	share(options: { url: string }): Promise<void>;
	/** Deletes every cached recording (sign-out: the next user must not play them). */
	clear(): Promise<void>;
}

/** Values kept in the app's Keychain (the user's own OpenAI key). */
export interface SecretsPlugin {
	get(options: { name: string }): Promise<{ value?: string }>;
	set(options: { name: string; value: string }): Promise<void>;
	remove(options: { name: string }): Promise<void>;
}

export const HoandatAuth = registerPlugin<HoandatAuthPlugin>("HoandatAuth");
export const NativeFiles = registerPlugin<NativeFilesPlugin>("NativeFiles");
export const Secrets = registerPlugin<SecretsPlugin>("Secrets");
