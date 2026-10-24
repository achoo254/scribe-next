import { activeTranslator } from "../i18n/active-translator";
/**
 * The one way the app talks to the backend. Paths start with `/api/`; on
 * the web they stay relative (same origin as nginx), in a native shell they
 * are prefixed with the backend origin because the page itself is served
 * from the app bundle. Every request sends the session cookie.
 */

/**
 * Origin of the backend when the page is not served by it (native shell).
 * Set at build time from web/.env (the build fails without it).
 */
export const NATIVE_API_ORIGIN: string =
	import.meta.env.VITE_NATIVE_API_ORIGIN ?? "";

/**
 * Numbers of a refused request that counts against an allowance (`403`
 * `{code, usedMinutes, limitMinutes, retryAt}`): what was used, the limit,
 * and when it opens again (ISO time; null when the backend does not say).
 */
export interface QuotaRefusal {
	used: number;
	limit: number;
	retryAt: string | null;
}

/** HTTP or network failure; `status` 0 means the request never got a reply. */
export class ApiError extends Error {
	readonly status: number;
	/** Machine-readable reason (`{code}` of the reply), e.g. `CHAT_NOT_LINKED`. */
	readonly code: string | null;
	readonly quota: QuotaRefusal | null;

	constructor(
		status: number,
		message: string,
		{
			code = null,
			quota = null,
		}: { code?: string | null; quota?: QuotaRefusal | null } = {},
	) {
		super(message);
		this.name = "ApiError";
		this.status = status;
		this.code = code;
		this.quota = quota;
	}
}

/** Listeners told when a reply says Scribe is off on this domain (403 `TENANT_DISABLED`). */
const tenantDisabledListeners = new Set<() => void>();

/**
 * Calls `listener` whenever a request is refused with `TENANT_DISABLED`
 * (the domain's owner or the operator turned Scribe off while the page was
 * open): lib/tenant-info.ts switches the app to the "not enabled" page.
 */
export function onTenantDisabled(listener: () => void): () => void {
	tenantDisabledListeners.add(listener);
	return () => tenantDisabledListeners.delete(listener);
}

/** `code` of a caught `ApiError`, else null. */
export function errorCode(error: unknown): string | null {
	return error instanceof ApiError ? error.code : null;
}

function field(body: object, name: string): unknown {
	return Reflect.get(body, name);
}

/** The quota numbers of a refusal body, or null when it carries none. */
function quotaOf(body: object): QuotaRefusal | null {
	const used = field(body, "usedMinutes");
	const limit = field(body, "limitMinutes");
	if (typeof used !== "number" || typeof limit !== "number") return null;
	const retryAt = field(body, "retryAt");
	return {
		used,
		limit,
		retryAt: typeof retryAt === "string" && retryAt ? retryAt : null,
	};
}

interface CapacitorGlobal {
	isNativePlatform?: () => boolean;
}

/** True inside a Capacitor shell (the global exists only there). */
export function isNativeShell(): boolean {
	const capacitor: unknown = Reflect.get(globalThis, "Capacitor");
	if (typeof capacitor !== "object" || capacitor === null) return false;
	const { isNativePlatform } = capacitor as CapacitorGlobal;
	return typeof isNativePlatform === "function" && isNativePlatform();
}

/** Empty on the web (relative URLs), the staging origin in a native shell. */
export function apiBase(native: boolean = isNativeShell()): string {
	return native ? NATIVE_API_ORIGIN : "";
}

/** Absolute or same-origin URL for an `/api/...` path. */
export function apiUrl(path: string, base: string = apiBase()): string {
	if (!path.startsWith("/api/")) {
		throw new Error(`apiUrl expects an /api/ path, got ${path}`);
	}
	return `${base}${path}`;
}

/**
 * Reads `{ error, code? }` (and the quota numbers) from a failed response;
 * the backend's text is Vietnamese.
 */
async function errorFrom(res: Response, fallback: string): Promise<ApiError> {
	const body: unknown = await res.json().catch(() => null);
	if (typeof body !== "object" || body === null) {
		return new ApiError(res.status, fallback);
	}
	const error = field(body, "error");
	const code = field(body, "code");
	return new ApiError(
		res.status,
		typeof error === "string" && error.length > 0 ? error : fallback,
		{
			code: typeof code === "string" && code ? code : null,
			quota: quotaOf(body),
		},
	);
}

export interface ApiRequest extends Omit<RequestInit, "body"> {
	/** JSON body (sent with Content-Type: application/json). */
	json?: unknown;
	/** Raw body, e.g. a chunk of a file. */
	body?: BodyInit;
	/** Message when a failed reply carries no `{ error }` of its own. */
	fallbackError?: string;
}

/**
 * The body as the native shell can send it. CapacitorHttp (which carries
 * every fetch in the iOS app) only encodes a `File` as binary: a plain
 * `Blob` such as `file.slice()` is refused before it leaves the phone, so
 * a chunk is wrapped in a `File` there. Same bytes, no copy on the web.
 * The File carries the request's `contentType`: Capacitor sends the
 * File's own type too, and a second, empty one would race with it.
 */
export function nativeSafeBody(
	body: BodyInit | null | undefined,
	contentType: string | null,
	native: boolean = isNativeShell(),
): BodyInit | null | undefined {
	if (native && body instanceof Blob && !(body instanceof File)) {
		return new File([body], "chunk", { type: contentType ?? body.type });
	}
	return body;
}

/**
 * Sends a request and returns the response when it is 2xx; anything else
 * throws an `ApiError` carrying the backend's message or `fallbackError`.
 */
export async function apiFetch(
	path: string,
	{
		json,
		fallbackError = activeTranslator().t("errors.requestFailed"),
		headers,
		...init
	}: ApiRequest = {},
	fetcher: typeof fetch = fetch,
): Promise<Response> {
	const requestHeaders = new Headers(headers);
	let body = nativeSafeBody(init.body, requestHeaders.get("Content-Type"));
	if (json !== undefined) {
		requestHeaders.set("Content-Type", "application/json");
		body = JSON.stringify(json);
	}
	let res: Response;
	try {
		res = await fetcher(apiUrl(path), {
			...init,
			body,
			headers: requestHeaders,
			credentials: "include",
		});
	} catch (cause) {
		if (cause instanceof DOMException && cause.name === "AbortError") {
			throw cause;
		}
		throw new ApiError(0, activeTranslator().t("errors.network"));
	}
	if (!res.ok) {
		const error = await errorFrom(res, fallbackError);
		if (error.code === "TENANT_DISABLED") {
			for (const notify of tenantDisabledListeners) notify();
		}
		throw error;
	}
	return res;
}

/** `apiFetch` plus the JSON body of the reply. */
export async function apiJson<T>(
	path: string,
	request: ApiRequest = {},
	fetcher: typeof fetch = fetch,
): Promise<T> {
	const res = await apiFetch(path, request, fetcher);
	return (await res.json()) as T;
}

/** Text to show for a caught error. */
export function errorMessage(
	error: unknown,
	fallback: string = activeTranslator().t("errors.generic"),
): string {
	if (error instanceof Error && error.message) return error.message;
	return fallback;
}
