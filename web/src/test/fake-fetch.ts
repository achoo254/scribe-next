import { vi } from "vitest";

export interface RecordedCall {
	method: string;
	/** Path and query, e.g. `/api/upload/chunk?session=s1&offset=0`. */
	url: string;
	body: unknown;
	credentials: RequestCredentials | undefined;
	contentType: string | null;
}

type Reply = Response | Error | ((call: RecordedCall) => Response | Error);

export function jsonResponse(body: unknown, status = 200): Response {
	return new Response(JSON.stringify(body), {
		status,
		headers: { "Content-Type": "application/json" },
	});
}

/**
 * A `fetch` stand-in: replies are queued per "METHOD path" (query ignored)
 * and consumed in order; the last reply of a route repeats. Unknown routes
 * fail the test.
 */
export function fakeFetch(routes: Record<string, Reply | Reply[]>) {
	const queues = new Map(
		Object.entries(routes).map(([key, reply]) => [
			key,
			Array.isArray(reply) ? [...reply] : [reply],
		]),
	);
	const calls: RecordedCall[] = [];
	const fetcher = vi.fn(
		async (input: RequestInfo | URL, init: RequestInit = {}) => {
			const url = String(input);
			const method = init.method ?? "GET";
			const headers = new Headers(init.headers);
			const call: RecordedCall = {
				method,
				url,
				body: typeof init.body === "string" ? JSON.parse(init.body) : init.body,
				credentials: init.credentials,
				contentType: headers.get("Content-Type"),
			};
			calls.push(call);
			const key = `${method} ${url.split("?")[0]}`;
			const queue = queues.get(key);
			if (!queue || queue.length === 0) {
				throw new Error(`Unexpected request ${key}`);
			}
			const next = queue.length > 1 ? queue.shift() : queue[0];
			const reply = typeof next === "function" ? next(call) : next;
			if (reply instanceof Error) throw reply;
			return reply instanceof Response ? reply.clone() : reply;
		},
	);
	return { fetcher: fetcher as unknown as typeof fetch, calls };
}
