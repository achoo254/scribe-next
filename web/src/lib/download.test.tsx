import { renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { NATIVE_API_ORIGIN } from "./api";
import { download, mediaUrl, useMediaSource } from "./download";

const { share, cache } = vi.hoisted(() => ({
	share: vi.fn(async () => undefined),
	cache: vi.fn(async () => ({
		url: "capacitor://localhost/_capacitor_file_/a.webm",
	})),
}));

vi.mock("./native", () => ({ NativeFiles: { share, cache } }));

function asNative(): void {
	Reflect.set(globalThis, "Capacitor", { isNativePlatform: () => true });
}

afterEach(() => {
	Reflect.deleteProperty(globalThis, "Capacitor");
	vi.clearAllMocks();
});

describe("download in the iOS app", () => {
	it("hands the absolute backend URL to the share sheet plugin", async () => {
		asNative();
		await download("/api/files/f1?download=1");
		expect(share).toHaveBeenCalledWith({
			url: `${NATIVE_API_ORIGIN}/api/files/f1?download=1`,
		});
	});
});

describe("useMediaSource", () => {
	it("streams straight from the API on the web", () => {
		const { result } = renderHook(() => useMediaSource("f1"));
		expect(result.current).toEqual({ src: mediaUrl("f1"), error: "" });
		expect(cache).not.toHaveBeenCalled();
	});

	it("plays the cached local copy in the iOS app", async () => {
		asNative();
		const { result } = renderHook(() => useMediaSource("f1"));
		expect(result.current.src).toBeNull();
		await waitFor(() =>
			expect(result.current.src).toBe(
				"capacitor://localhost/_capacitor_file_/a.webm",
			),
		);
		expect(cache).toHaveBeenCalledWith({
			url: `${NATIVE_API_ORIGIN}/api/files/f1`,
		});
	});

	it("never hands out the previous file right after the file changes", async () => {
		asNative();
		const seen: Array<{ fileId: string; src: string | null }> = [];
		const { result, rerender } = renderHook(
			({ fileId }) => {
				const source = useMediaSource(fileId);
				seen.push({ fileId, src: source.src });
				return source;
			},
			{ initialProps: { fileId: "f1" } },
		);
		await waitFor(() => expect(result.current.src).not.toBeNull());
		cache.mockImplementationOnce(() => new Promise(() => undefined));
		rerender({ fileId: "f2" });
		// Not even one render of f2 may carry the f1 copy.
		expect(seen.filter((r) => r.fileId === "f2").map((r) => r.src)).toEqual(
			seen.filter((r) => r.fileId === "f2").map(() => null),
		);
	});

	it("switches the web stream URL in the same render", () => {
		const seen: Array<string | null> = [];
		const { rerender } = renderHook(
			({ fileId }) => {
				const source = useMediaSource(fileId);
				if (fileId === "f2") seen.push(source.src);
				return source;
			},
			{ initialProps: { fileId: "f1" } },
		);
		rerender({ fileId: "f2" });
		expect(new Set(seen)).toEqual(new Set([mediaUrl("f2")]));
	});

	it("reports a failed download", async () => {
		asNative();
		cache.mockRejectedValueOnce(new Error("Phiên đăng nhập đã hết"));
		const { result } = renderHook(() => useMediaSource("f2"));
		await waitFor(() =>
			expect(result.current.error).toBe("Phiên đăng nhập đã hết"),
		);
	});
});
