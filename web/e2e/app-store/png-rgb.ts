import { deflateSync, inflateSync } from "node:zlib";

/*
 * App Store Connect refuses screenshots with an alpha channel, and
 * Playwright writes RGBA PNGs. This rewrites such a PNG (8-bit RGBA,
 * not interlaced: what Playwright makes) as an 8-bit RGB PNG with the same
 * pixels, choosing each row's filter the usual way (smallest sum of
 * absolute differences) so the file stays small.
 */

const SIGNATURE = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);

interface Chunk {
	type: string;
	data: Buffer;
}

function readChunks(png: Buffer): Chunk[] {
	if (!png.subarray(0, 8).equals(SIGNATURE)) throw new Error("not a PNG");
	const chunks: Chunk[] = [];
	let at = 8;
	while (at < png.length) {
		const length = png.readUInt32BE(at);
		const type = png.toString("latin1", at + 4, at + 8);
		chunks.push({ type, data: png.subarray(at + 8, at + 8 + length) });
		at += 12 + length;
	}
	return chunks;
}

const CRC_TABLE = Array.from({ length: 256 }, (_, n) => {
	let c = n;
	for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
	return c >>> 0;
});

function crc32(bytes: Buffer): number {
	let c = 0xffffffff;
	for (const byte of bytes) c = (CRC_TABLE[(c ^ byte) & 0xff] ?? 0) ^ (c >>> 8);
	return (c ^ 0xffffffff) >>> 0;
}

function chunk(type: string, data: Buffer): Buffer {
	const head = Buffer.alloc(8);
	head.writeUInt32BE(data.length, 0);
	head.write(type, 4, "latin1");
	const crc = Buffer.alloc(4);
	crc.writeUInt32BE(crc32(Buffer.concat([head.subarray(4), data])), 0);
	return Buffer.concat([head, data, crc]);
}

function paeth(a: number, b: number, c: number): number {
	const p = a + b - c;
	const pa = Math.abs(p - a);
	const pb = Math.abs(p - b);
	const pc = Math.abs(p - c);
	if (pa <= pb && pa <= pc) return a;
	return pb <= pc ? b : c;
}

/** Undoes the row filters: raw pixel rows, `bpp` bytes per pixel. */
function unfilter(
	data: Buffer,
	width: number,
	height: number,
	bpp: number,
): Buffer {
	const stride = width * bpp;
	const out = Buffer.alloc(stride * height);
	for (let y = 0; y < height; y++) {
		const type = data[y * (stride + 1)];
		const src = y * (stride + 1) + 1;
		const row = y * stride;
		for (let x = 0; x < stride; x++) {
			const raw = data[src + x] ?? 0;
			const a = x >= bpp ? (out[row + x - bpp] ?? 0) : 0;
			const b = y > 0 ? (out[row - stride + x] ?? 0) : 0;
			const c = x >= bpp && y > 0 ? (out[row - stride + x - bpp] ?? 0) : 0;
			const predictor =
				type === 0
					? 0
					: type === 1
						? a
						: type === 2
							? b
							: type === 3
								? (a + b) >> 1
								: paeth(a, b, c);
			out[row + x] = (raw + predictor) & 0xff;
		}
	}
	return out;
}

/** Filters rows of `pixels` (`bpp` bytes per pixel), picking the cheapest filter per row. */
function filter(
	pixels: Buffer,
	width: number,
	height: number,
	bpp: number,
): Buffer {
	const stride = width * bpp;
	const out = Buffer.alloc((stride + 1) * height);
	const candidate = Buffer.alloc(stride);
	for (let y = 0; y < height; y++) {
		const row = y * stride;
		let best = Number.POSITIVE_INFINITY;
		for (let type = 0; type <= 4; type++) {
			let cost = 0;
			for (let x = 0; x < stride; x++) {
				const value = pixels[row + x] ?? 0;
				const a = x >= bpp ? (pixels[row + x - bpp] ?? 0) : 0;
				const b = y > 0 ? (pixels[row - stride + x] ?? 0) : 0;
				const c = x >= bpp && y > 0 ? (pixels[row - stride + x - bpp] ?? 0) : 0;
				const predictor =
					type === 0
						? 0
						: type === 1
							? a
							: type === 2
								? b
								: type === 3
									? (a + b) >> 1
									: paeth(a, b, c);
				const filtered = (value - predictor) & 0xff;
				candidate[x] = filtered;
				cost += filtered < 128 ? filtered : 256 - filtered;
			}
			if (cost < best) {
				best = cost;
				out[y * (stride + 1)] = type;
				candidate.copy(out, y * (stride + 1) + 1);
			}
		}
	}
	return out;
}

/** The same image without its alpha channel (alpha must be opaque: a page screenshot). */
export function toRgbPng(png: Buffer): Buffer {
	const chunks = readChunks(png);
	const header = chunks.find((c) => c.type === "IHDR")?.data;
	if (!header) throw new Error("PNG without IHDR");
	const width = header.readUInt32BE(0);
	const height = header.readUInt32BE(4);
	const [depth, colour, , , interlace] = header.subarray(8, 13);
	if (depth !== 8 || interlace !== 0 || (colour !== 6 && colour !== 2)) {
		throw new Error(
			`unsupported PNG (depth ${depth}, colour ${colour}, interlace ${interlace})`,
		);
	}
	if (colour === 2) return png;
	const idat = Buffer.concat(
		chunks.filter((c) => c.type === "IDAT").map((c) => c.data),
	);
	const rgba = unfilter(inflateSync(idat), width, height, 4);
	const rgb = Buffer.alloc(width * height * 3);
	for (let i = 0, j = 0; i < rgba.length; i += 4, j += 3) {
		if (rgba[i + 3] !== 0xff)
			throw new Error("screenshot has transparent pixels");
		rgb[j] = rgba[i] ?? 0;
		rgb[j + 1] = rgba[i + 1] ?? 0;
		rgb[j + 2] = rgba[i + 2] ?? 0;
	}
	const ihdr = Buffer.from(header);
	ihdr[9] = 2;
	return Buffer.concat([
		SIGNATURE,
		chunk("IHDR", ihdr),
		chunk("IDAT", deflateSync(filter(rgb, width, height, 3), { level: 9 })),
		chunk("IEND", Buffer.alloc(0)),
	]);
}
