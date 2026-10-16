import { describe, expect, it } from "vitest";
import type { ManagedFile, RecordingItem } from "./files-api";
import {
	daysUntilExpiry,
	expiresSoon,
	expiryLabel,
	groupByDay,
	groupOutputsByJob,
	groupSessions,
	parseSegmentName,
	sessionTitle,
} from "./recording-groups";

function recording(id: string, createdAt?: string): RecordingItem {
	return { id, name: id, hasVideo: false, size: 1, createdAt };
}

describe("groupByDay", () => {
	it("groups by local day, keeps the backend order and spans [midnight, next midnight)", () => {
		const groups = groupByDay([
			recording("a", new Date(2026, 9, 15, 18, 0).toISOString()),
			recording("b", new Date(2026, 9, 15, 8, 0).toISOString()),
			recording("c", new Date(2026, 9, 14, 23, 59).toISOString()),
		]);
		expect(groups.map((group) => group.items.map((item) => item.id))).toEqual([
			["a", "b"],
			["c"],
		]);
		expect(groups[0]?.from).toBe(new Date(2026, 9, 15).getTime());
		expect(groups[0]?.to).toBe(new Date(2026, 9, 16).getTime());
	});

	it("puts recordings without a valid date together", () => {
		const groups = groupByDay([recording("a"), recording("b", "not a date")]);
		expect(groups).toHaveLength(1);
	});
});

describe("expiry", () => {
	const now = new Date("2026-10-15T00:00:00Z").getTime();

	it("counts whole days left, rounded up", () => {
		expect(daysUntilExpiry("2026-10-10T00:00:00Z", 14, now)).toBe(9);
		expect(daysUntilExpiry("2026-10-01T12:00:00Z", 14, now)).toBe(1);
	});

	it("is unknown without retention or date", () => {
		expect(daysUntilExpiry("2026-10-10T00:00:00Z", 0, now)).toBeNull();
		expect(daysUntilExpiry(undefined, 14, now)).toBeNull();
		expect(expiryLabel(null)).toBe("");
	});

	it("labels and highlights the last three days", () => {
		expect(expiryLabel(0)).toBe("sắp tự xoá");
		expect(expiryLabel(1)).toBe("tự xoá sau 1 ngày");
		expect(expiryLabel(9)).toBe("tự xoá sau 9 ngày");
		expect(expiresSoon(3)).toBe(true);
		expect(expiresSoon(4)).toBe(false);
	});
});

describe("groupOutputsByJob", () => {
	it("sums segments per job and ignores uploads", () => {
		const files: ManagedFile[] = [
			{ id: "u1", name: "src.mp4", kind: "upload", size: 100 },
			{ id: "o1", name: "a", kind: "output", size: 10, jobId: "j1" },
			{ id: "o2", name: "b", kind: "output", size: 20, jobId: "j1" },
			{ id: "o3", name: "c", kind: "output", size: 5, jobId: "j2" },
		];
		expect(groupOutputsByJob(files)).toEqual([
			{
				jobId: "j1",
				count: 2,
				bytes: 30,
				createdAt: undefined,
				ids: ["o1", "o2"],
			},
			{ jobId: "j2", count: 1, bytes: 5, createdAt: undefined, ids: ["o3"] },
		]);
	});
});

describe("groupSessions", () => {
	const seg = (session: string, index: number, at: string, duration = 60) => ({
		id: `${session}-${index}`,
		name: `${session}#${index}`,
		hasVideo: false,
		size: 100,
		durationS: duration,
		createdAt: at,
	});

	it("collapses iOS segments of one session into one row, in recording order", () => {
		const rows = groupSessions([
			seg("aaaaaaaa-1111", 2, "2026-10-15T10:03:00Z", 30),
			{
				id: "web",
				name: "Bản ghi 16:31:09",
				hasVideo: false,
				size: 5,
				createdAt: "2026-10-15T09:31:00Z",
			},
			seg("aaaaaaaa-1111", 0, "2026-10-15T10:01:00Z"),
			seg("aaaaaaaa-1111", 1, "2026-10-15T10:02:00Z"),
		]);
		expect(rows.map((row) => row.kind)).toEqual(["session", "single"]);
		const session = rows[0];
		if (session?.kind !== "session") throw new Error("expected a session row");
		expect(session.parts.map((part) => part.index)).toEqual([0, 1, 2]);
		expect(session.durationS).toBe(150);
		expect(session.size).toBe(300);
		expect(session.createdAt).toBe("2026-10-15T10:01:00Z");
	});

	it("keeps names that only look similar as single recordings", () => {
		expect(parseSegmentName("Bản ghi #3")).toBeNull();
		expect(parseSegmentName("abc#1")).toBeNull();
		// Same limits as the backend's session key.
		expect(parseSegmentName(`${"a".repeat(65)}#1`)).toBeNull();
		expect(parseSegmentName("aaaaaaaa-1111#1234567890")).toBeNull();
		expect(parseSegmentName("6ad09d56-a5e5#12")).toEqual({
			sessionId: "6ad09d56-a5e5",
			index: 12,
		});
	});
});

describe("sessionTitle", () => {
	it("names a session by its start time, with the date when asked", () => {
		const start = new Date(2026, 9, 16, 9, 41).toISOString();
		expect(sessionTitle(start)).toBe("Bản ghi trên iPhone 09:41");
		expect(sessionTitle(start, { withDate: true })).toBe(
			"Bản ghi trên iPhone 09:41 16/10/2026",
		);
	});

	it("falls back to a plain name without a valid start", () => {
		expect(sessionTitle(undefined)).toBe("Bản ghi trên iPhone");
		expect(sessionTitle("not a date", { withDate: true })).toBe(
			"Bản ghi trên iPhone",
		);
	});
});
