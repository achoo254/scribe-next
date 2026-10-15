/**
 * The one way the app talks to the backend. Paths start with `/api/`; on
 * the web they stay relative (same origin as nginx), in a native shell they
 * are prefixed with the staging origin because the page itself is served
 * from the app bundle. Every request sends the session cookie.
 */

/** Origin of the backend when the page is not served by it (native shell). */
export const NATIVE_API_ORIGIN = "https://scribe.inetdev.io.vn";

const NETWORK_ERROR_MESSAGE =
	"Không kết nối được máy chủ — kiểm tra mạng rồi thử lại.";

/** HTTP or network failure; `status` 0 means the request never got a reply. */
export class ApiError extends Error {
	readonly status: number;

	constructor(status: number, message: string) {
		super(message);
		this.name = "ApiError";
		this.status = status;
	}
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

/** Reads `{ error }` from a failed response; the backend's text is Vietnamese. */
async function errorFrom(res: Response, fallback: string): Promise<ApiError> {
	const body: unknown = await res.json().catch(() => null);
	const message =
		typeof body === "object" &&
		body !== null &&
		"error" in body &&
		typeof body.error === "string" &&
		body.error.length > 0
			? body.error
			: fallback;
	return new ApiError(res.status, message);
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
 * Sends a request and returns the response when it is 2xx; anything else
 * throws an `ApiError` carrying the backend's message or `fallbackError`.
 */
export async function apiFetch(
	path: string,
	{
		json,
		fallbackError = "Yêu cầu thất bại",
		headers,
		...init
	}: ApiRequest = {},
	fetcher: typeof fetch = fetch,
): Promise<Response> {
	const requestHeaders = new Headers(headers);
	let body = init.body;
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
		throw new ApiError(0, NETWORK_ERROR_MESSAGE);
	}
	if (!res.ok) throw await errorFrom(res, fallbackError);
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
	fallback = "Đã có lỗi xảy ra.",
): string {
	if (error instanceof Error && error.message) return error.message;
	return fallback;
}
