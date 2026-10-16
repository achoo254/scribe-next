import type { Page } from "@playwright/test";
import { E2E_STUB_MARKER, NATIVE_STUB_GLOBAL } from "./stub-markers";

/** One call the app made to a native plugin. */
export interface NativeCall {
	plugin: string;
	method: string;
	options: unknown;
}

/** What `Recorder.getStatus` answers (web/src/features/recorder/native-recorder.ts). */
export interface NativeRecorderStatus {
	state: "idle" | "starting" | "recording" | "stopping";
	elapsedS: number;
	uploadsWaiting: number;
	uploadsFailed: number;
	notice: string;
}

export const IDLE_RECORDER: NativeRecorderStatus = {
	state: "idle",
	elapsedS: 0,
	uploadsWaiting: 0,
	uploadsFailed: 0,
	notice: "",
};

interface StubOptions {
	marker: string;
	globalName: string;
	email: string;
	status: NativeRecorderStatus;
}

/**
 * Runs in the page before the app (`addInitScript`, so it is serialised:
 * it may only use its argument). Makes Capacitor believe it runs in the
 * iOS shell, the way the real bridge does: `webkit.messageHandlers.bridge`
 * selects the "ios" platform and `PluginHeaders` + `nativePromise` /
 * `nativeCallback` answer the plugin calls of web/src/lib/native.ts and
 * features/recorder/native-recorder.ts with a recorder in `status` (idle
 * unless a spec says otherwise).
 */
function installCapacitorStub({
	marker,
	globalName,
	email,
	status,
}: StubOptions) {
	const results: Record<string, unknown> = {
		"Recorder.getStatus": status,
		"Recorder.stop": { ...status, state: "idle" },
		"Recorder.start": { sessionId: "e2e-session" },
		"Recorder.syncUploads": { lostSegments: 0, lostSecondsMax: 0 },
		"HoandatAuth.signIn": { email },
		"NativeFiles.cache": { url: "" },
		"Secrets.get": {},
	};
	const promiseMethods: Record<string, string[]> = {
		Recorder: [
			"start",
			"stop",
			"getStatus",
			"syncUploads",
			"signOut",
			"removeListener",
			"removeAllListeners",
		],
		HoandatAuth: ["signIn", "signOut"],
		NativeFiles: ["cache", "share", "clear"],
		Secrets: ["get", "set", "remove"],
	};
	const calls: Array<{ plugin: string; method: string; options: unknown }> = [];
	let callbackId = 0;

	Object.defineProperty(window, "webkit", {
		configurable: true,
		value: { messageHandlers: { bridge: { postMessage: () => undefined } } },
	});
	Object.defineProperty(window, "Capacitor", {
		configurable: true,
		writable: true,
		value: {
			PluginHeaders: Object.entries(promiseMethods).map(([name, methods]) => ({
				name,
				methods: [
					...methods.map((method) => ({ name: method, rtype: "promise" })),
					{ name: "addListener", rtype: "callback" },
				],
			})),
			nativePromise: (plugin: string, method: string, options: unknown) => {
				calls.push({ plugin, method, options });
				return Promise.resolve(results[`${plugin}.${method}`]);
			},
			// Listeners never fire: the stub recorder keeps its status.
			nativeCallback: (plugin: string, method: string, options: unknown) => {
				calls.push({ plugin, method, options });
				callbackId += 1;
				return String(callbackId);
			},
		},
	});
	Object.defineProperty(window, globalName, {
		configurable: true,
		value: { marker, calls },
	});
}

/** Renders the page as the iOS app does (native branch of the UI). */
export async function stubNativeShell(
	page: Page,
	email: string,
	status: NativeRecorderStatus = IDLE_RECORDER,
): Promise<void> {
	await page.addInitScript(installCapacitorStub, {
		marker: E2E_STUB_MARKER,
		globalName: NATIVE_STUB_GLOBAL,
		email,
		status,
	});
}

function isNativeCall(value: unknown): value is NativeCall {
	return (
		typeof value === "object" &&
		value !== null &&
		"plugin" in value &&
		typeof value.plugin === "string" &&
		"method" in value &&
		typeof value.method === "string"
	);
}

/** Plugin calls the app made so far. */
export async function nativeCalls(page: Page): Promise<NativeCall[]> {
	const raw: unknown = await page.evaluate(
		(name) => Reflect.get(Reflect.get(window, name) ?? {}, "calls"),
		NATIVE_STUB_GLOBAL,
	);
	if (!Array.isArray(raw))
		throw new Error("The Capacitor stub is not installed");
	return raw.filter(isNativeCall);
}
