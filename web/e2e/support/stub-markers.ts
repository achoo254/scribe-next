/**
 * Names only the e2e stubs use. `check-dist.ts` fails the production build
 * if any of them reaches `dist/`, i.e. if app code ever imports a stub.
 */
export const E2E_STUB_MARKER = "scribe-e2e-stub";

/** Window property where the Capacitor stub records the plugin calls. */
export const NATIVE_STUB_GLOBAL = "__scribeE2eNative";

export const E2E_STUB_MARKERS: readonly string[] = [
	E2E_STUB_MARKER,
	NATIVE_STUB_GLOBAL,
];
