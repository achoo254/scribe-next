// Mode A — Ghi trong browser: getUserMedia (mic) / getDisplayMedia (màn hình + system audio) →
// MediaRecorder timeslice 1s → upload chunk nền (streaming append, không mất khi dừng đột ngột) →
// finalize → files{kind:recording}. Track thời gian bằng timer riêng (KHÔNG đếm chunk).
//
// Capture hẹp hơn desktop: system audio chỉ Chrome/Edge + chỉ khi user chọn "Toàn màn hình"; không
// có chọn vùng. Firefox/Safari → không system audio (fallback mic). Disclosure ở RecordView.
import { ref } from "vue";

export type RecordSource = "mic" | "screen";

export interface RecordResult {
  fileId: string;
  name: string;
  durationS: number;
}

const CHUNK_RETRY = 3;
// Cap bitrate → 1 chunk timeslice 1s luôn ≪ body limit 8MB kể cả màn hình 4K (tránh 413 làm mất ghi).
const SCREEN_VIDEO_BPS = 2_500_000;
const AUDIO_BPS = 128_000;

const sleep = (ms: number): Promise<void> => new Promise((r) => setTimeout(r, ms));

/** Browser hỗ trợ getDisplayMedia (ghi màn hình) — Chrome/Edge; Firefox có nhưng system audio kém. */
export function supportsScreenCapture(): boolean {
  return typeof navigator !== "undefined" && !!navigator.mediaDevices?.getDisplayMedia;
}

/** Chọn mimeType webm tốt nhất browser hỗ trợ cho nguồn. */
function pickMime(source: RecordSource): string {
  const candidates =
    source === "mic"
      ? ["audio/webm;codecs=opus", "audio/webm"]
      : ["video/webm;codecs=vp9,opus", "video/webm;codecs=vp8,opus", "video/webm"];
  for (const c of candidates) {
    if (MediaRecorder.isTypeSupported(c)) return c;
  }
  return source === "mic" ? "audio/webm" : "video/webm";
}

export function useRecorder() {
  const recording = ref(false);
  const starting = ref(false);
  const finalizing = ref(false);
  const elapsedS = ref(0);
  const audioLevel = ref(0); // RMS âm thanh 0..1, cập nhật realtime cho level meter
  const previewStream = ref<MediaStream | null>(null); // stream để <video> preview gắn srcObject
  const error = ref("");

  let recorder: MediaRecorder | null = null;
  let stream: MediaStream | null = null;
  let audioCtx: AudioContext | null = null;
  let analyser: AnalyserNode | null = null;
  let meterRaf: number | null = null;
  let timer: number | null = null;
  let sessionId: string | null = null;
  let offset = 0;
  let uploadChain: Promise<void> = Promise.resolve();
  let chainError: string | null = null;
  let recordName = "";

  /**
   * Gắn AnalyserNode đọc RMS âm thanh realtime cho level meter. Dùng chung audioCtx với bước mix;
   * KHÔNG nối analyser ra loa (ctx.destination) nên không phát tiếng, chỉ đo mức.
   */
  function attachMeter(tracks: MediaStreamTrack[]): void {
    if (tracks.length === 0) return;
    audioCtx ??= new AudioContext();
    analyser = audioCtx.createAnalyser();
    analyser.fftSize = 512;
    for (const t of tracks) {
      audioCtx.createMediaStreamSource(new MediaStream([t])).connect(analyser);
    }
    const buf = new Uint8Array(analyser.fftSize);
    const tick = (): void => {
      if (!analyser) return;
      analyser.getByteTimeDomainData(buf);
      let sum = 0;
      for (let i = 0; i < buf.length; i++) {
        const v = (buf[i] - 128) / 128;
        sum += v * v;
      }
      audioLevel.value = Math.sqrt(sum / buf.length); // RMS 0..1
      meterRaf = requestAnimationFrame(tick);
    };
    tick();
  }

  /** Lấy stream theo nguồn; screen = video (≤30fps) + mix (system audio nếu có) + mic. */
  async function acquireStream(source: RecordSource): Promise<MediaStream> {
    if (source === "mic") {
      const micStream = await navigator.mediaDevices.getUserMedia({ audio: true });
      attachMeter(micStream.getAudioTracks());
      return micStream;
    }
    const display = await navigator.mediaDevices.getDisplayMedia({
      video: { frameRate: { ideal: 30, max: 30 } },
      audio: true,
    });
    let mic: MediaStream | null = null;
    try {
      mic = await navigator.mediaDevices.getUserMedia({ audio: true });
    } catch {
      mic = null; // không có mic / bị từ chối → chỉ system audio
    }
    const audioTracks = [...display.getAudioTracks(), ...(mic?.getAudioTracks() ?? [])];
    const videoTrack = display.getVideoTracks()[0];
    // Dừng ghi khi user bấm "Stop sharing" của browser.
    videoTrack?.addEventListener("ended", () => void stop());
    // Meter đọc từ các track gốc → đúng mức dù có mix hay không.
    attachMeter(audioTracks);
    const videoOnly = videoTrack ? [videoTrack] : [];

    if (audioTracks.length <= 1) {
      return new MediaStream([...videoOnly, ...audioTracks]);
    }
    // ≥2 nguồn audio (system + mic) → mix qua Web Audio thành 1 track (tái dùng audioCtx của meter).
    const ctx = (audioCtx ??= new AudioContext());
    const dest = ctx.createMediaStreamDestination();
    for (const t of audioTracks) {
      ctx.createMediaStreamSource(new MediaStream([t])).connect(dest);
    }
    const mixed = dest.stream.getAudioTracks()[0];
    return new MediaStream([...videoOnly, mixed]);
  }

  async function postJson<T>(url: string, body: unknown): Promise<T> {
    const res = await fetch(url, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      credentials: "include",
      body: JSON.stringify(body),
    });
    if (!res.ok) {
      const b = (await res.json().catch(() => null)) as { error?: string } | null;
      throw new Error(b?.error ?? "Yêu cầu thất bại");
    }
    return (await res.json()) as T;
  }

  /** Upload 1 chunk tại offset hiện tại (retry + backoff); cập nhật offset theo BE trả về. */
  async function uploadChunk(sid: string, blob: Blob): Promise<void> {
    // Bỏ chunk nếu phiên đã đổi (race) hoặc đã có lỗi chuỗi / chunk rỗng.
    if (chainError || sid !== sessionId || blob.size === 0) return;
    const buf = await blob.arrayBuffer();
    for (let attempt = 0; attempt < CHUNK_RETRY; attempt++) {
      try {
        const res = await fetch(`/api/recordings/chunk?session=${sid}&offset=${offset}`, {
          method: "POST",
          credentials: "include",
          headers: { "Content-Type": "application/octet-stream" },
          body: buf,
        });
        if (!res.ok) {
          const b = (await res.json().catch(() => null)) as { error?: string } | null;
          throw new Error(b?.error ?? "Tải chunk thất bại");
        }
        const body = (await res.json()) as { written: number };
        offset = body.written;
        return;
      } catch (e) {
        if (attempt === CHUNK_RETRY - 1) {
          // Đánh dấu lỗi chuỗi → dừng gửi chunk sau; stop() vẫn finalize phần đã tải được.
          chainError = e instanceof Error ? e.message : "Lỗi tải bản ghi";
        } else {
          await sleep(300 * (attempt + 1)); // backoff cho lỗi mạng thoáng qua
        }
      }
    }
  }

  async function start(source: RecordSource): Promise<void> {
    // Reentrancy guard: chặn start khi đang ghi / đang khởi động / đang finalize phiên trước.
    if (recording.value || starting.value || finalizing.value) return;
    starting.value = true;
    error.value = "";
    chainError = null;
    offset = 0;
    elapsedS.value = 0;
    recordName = `Bản ghi ${new Date().toLocaleString("vi-VN")}`;
    try {
      const init = await postJson<{ sessionId: string }>("/api/recordings/init", { name: recordName });
      sessionId = init.sessionId;
      const sid = init.sessionId;
      stream = await acquireStream(source);
      previewStream.value = stream;
      const options: MediaRecorderOptions = { mimeType: pickMime(source), audioBitsPerSecond: AUDIO_BPS };
      if (source === "screen") options.videoBitsPerSecond = SCREEN_VIDEO_BPS;
      recorder = new MediaRecorder(stream, options);
      recorder.ondataavailable = (e) => {
        if (e.data && e.data.size > 0) {
          uploadChain = uploadChain.then(() => uploadChunk(sid, e.data));
        }
      };
      recorder.onerror = () => {
        error.value = "Lỗi ghi (MediaRecorder) — đã dừng";
        void stop();
      };
      recorder.start(1000); // timeslice 1s — upload nền
      recording.value = true;
      timer = window.setInterval(() => (elapsedS.value += 1), 1000);
    } catch (e) {
      error.value = e instanceof Error ? e.message : "Không bắt đầu ghi được (quyền truy cập bị từ chối?)";
      cleanup();
      sessionId = null;
    } finally {
      starting.value = false;
    }
  }

  /** Dừng ghi → chờ upload drain → finalize (best-effort kể cả khi có lỗi chunk). */
  async function stop(): Promise<RecordResult | null> {
    if (!recorder || !recording.value) return null;
    recording.value = false;
    if (timer !== null) {
      clearInterval(timer);
      timer = null;
    }
    // Chờ MediaRecorder flush chunk cuối rồi đóng.
    await new Promise<void>((resolve) => {
      recorder!.onstop = () => resolve();
      recorder!.stop();
    });
    cleanupStream();

    finalizing.value = true;
    try {
      await uploadChain; // drain hàng đợi chunk
      if (!sessionId) throw new Error("Phiên ghi không hợp lệ");
      // Best-effort: dù có chunk lỗi giữa chừng, vẫn finalize phần đã tải để KHÔNG mất cả bản ghi.
      // Gửi kèm durationS (timer client) vì webm MediaRecorder thường thiếu duration metadata.
      const result = await postJson<RecordResult>("/api/recordings/finalize", {
        session: sessionId,
        durationS: elapsedS.value,
      });
      if (chainError) {
        error.value = "Một số đoạn bị lỗi mạng — đã lưu phần tải được (bản ghi có thể bị cắt ngắn).";
      }
      return result;
    } catch (e) {
      error.value = e instanceof Error ? e.message : "Không hoàn tất bản ghi";
      return null;
    } finally {
      finalizing.value = false;
      sessionId = null;
      recorder = null;
    }
  }

  function cleanupStream(): void {
    if (meterRaf !== null) {
      cancelAnimationFrame(meterRaf);
      meterRaf = null;
    }
    analyser = null;
    audioLevel.value = 0;
    previewStream.value = null;
    stream?.getTracks().forEach((t) => t.stop());
    stream = null;
    if (audioCtx) {
      void audioCtx.close().catch(() => {});
      audioCtx = null;
    }
  }

  function cleanup(): void {
    if (timer !== null) {
      clearInterval(timer);
      timer = null;
    }
    cleanupStream();
    recorder = null;
    recording.value = false;
  }

  /** Dọn tài nguyên khi component huỷ (mic/màn hình không bật ngầm). */
  function dispose(): void {
    cleanup();
  }

  return { recording, starting, finalizing, elapsedS, audioLevel, previewStream, error, start, stop, dispose };
}
