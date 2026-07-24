// Mode A — Ghi trong browser: 3 nguồn độc lập bật/tắt riêng — mic (getUserMedia), tiếng hệ thống +
// màn hình (getDisplayMedia). Nhiều nguồn audio → trộn qua Web Audio thành 1 track. Có video track
// (bật ghi màn hình) → xuất video/webm, ngược lại audio/webm. MediaRecorder timeslice 1s → upload
// chunk nền (streaming append, không mất khi dừng đột ngột) → finalize → files{kind:recording}.
// Track thời gian bằng timer riêng (KHÔNG đếm chunk).
//
// Ràng buộc web: KHÔNG lấy được tiếng hệ thống mà không chia sẻ màn hình — getDisplayMedia luôn kèm
// video track + hộp chọn của browser (không có loopback thuần như desktop). Chọn "tiếng hệ thống" mà
// tắt "ghi màn hình" → bỏ video track, chỉ lưu audio. System audio tốt nhất trên Chrome/Edge; nếu
// browser không cấp thì nguồn đó rỗng. Disclosure ở RecordView.
import { ref } from "vue";

/** Các nguồn ghi bật/tắt độc lập; ≥1 nguồn phải bật (RecordView chặn khi rỗng). */
export interface RecordSources {
  mic: boolean;
  systemAudio: boolean;
  screen: boolean;
}

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

/** Chọn mimeType webm tốt nhất browser hỗ trợ (video khi có màn hình, ngược lại audio). */
function pickMime(hasVideo: boolean): string {
  const candidates = hasVideo
    ? ["video/webm;codecs=vp9,opus", "video/webm;codecs=vp8,opus", "video/webm"]
    : ["audio/webm;codecs=opus", "audio/webm"];
  for (const c of candidates) {
    if (MediaRecorder.isTypeSupported(c)) return c;
  }
  return hasVideo ? "video/webm" : "audio/webm";
}

/**
 * Chỉ đường cấp quyền Micro đúng nơi theo trình duyệt (nhận diện qua UA). Web KHÔNG mở được Cài đặt
 * hệ thống → chỉ hướng dẫn được. Edge/Chrome/Firefox iOS quản lý quyền mic ở cấp iOS Settings của app;
 * Safari iOS ở "aA" → Cài đặt Trang web; desktop/Android ở biểu tượng quyền trên thanh địa chỉ.
 */
function micPermissionHint(): string {
  const nav = typeof navigator !== "undefined" ? navigator : undefined;
  const ua = nav?.userAgent ?? "";
  const isIOS = /iPad|iPhone|iPod/.test(ua) || (ua.includes("Macintosh") && (nav?.maxTouchPoints ?? 0) > 1);
  if (isIOS) {
    if (ua.includes("EdgiOS")) return "Vào Cài đặt iOS → Edge → Micro → bật, rồi tải lại trang và thử lại.";
    if (ua.includes("CriOS")) return "Vào Cài đặt iOS → Chrome → Micro → bật, rồi tải lại trang và thử lại.";
    if (ua.includes("FxiOS")) return "Vào Cài đặt iOS → Firefox → Micro → bật, rồi tải lại trang và thử lại.";
    return "Bấm nút “aA” bên trái thanh địa chỉ → Cài đặt Trang web → Micro → Cho phép (hoặc Cài đặt iOS → Safari → Micro), rồi tải lại trang.";
  }
  return "Bấm biểu tượng khoá/quyền bên trái thanh địa chỉ → cho phép Micro, rồi tải lại trang và thử lại.";
}

/**
 * Đổi lỗi khi bắt đầu ghi sang thông báo tiếng Việt actionable. getUserMedia/getDisplayMedia reject
 * bằng DOMException có tên chuẩn — riêng NotAllowedError (quyền bị từ chối / dialog bị huỷ) là ca hay
 * gặp trên WebKit iOS nên hướng dẫn cấp quyền cụ thể. Lỗi khác (BE init, mạng) giữ nguyên message gốc.
 */
function startErrorMessage(e: unknown, src: RecordSources): string {
  if (e instanceof DOMException) {
    switch (e.name) {
      case "NotAllowedError":
        return src.screen || src.systemAudio
          ? "Chưa được cấp quyền chia sẻ màn hình / micro — hãy chọn nguồn chia sẻ và cho phép truy cập, rồi thử lại."
          : `Trình duyệt chưa được cấp quyền Micro. ${micPermissionHint()}`;
      case "NotFoundError":
      case "OverconstrainedError":
        return "Không tìm thấy thiết bị micro phù hợp.";
      case "NotReadableError":
        return "Micro đang được ứng dụng khác sử dụng — hãy đóng ứng dụng đó rồi thử lại.";
      case "AbortError":
        return "Không truy cập được micro (thiết bị bận hoặc bị gián đoạn) — thử lại.";
      case "SecurityError":
        return "Ghi âm cần kết nối bảo mật (HTTPS).";
      default:
        return e.message || "Không bắt đầu ghi được.";
    }
  }
  return e instanceof Error ? e.message : "Không bắt đầu ghi được (quyền truy cập bị từ chối?)";
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
  let wakeLock: WakeLockSentinel | null = null;
  let interruptNote: string | null = null; // lý do dừng bất thường (khóa màn hình…) → hiển thị sau finalize

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

  /**
   * Lấy stream theo các nguồn bật/tắt độc lập. getDisplayMedia gọi trước (nhạy user-activation hơn)
   * phục vụ cả tiếng hệ thống lẫn video màn hình; nếu chỉ cần audio thì bỏ video track. Nhiều nguồn
   * audio → trộn qua Web Audio thành 1 track. Trả stream rỗng nếu không nguồn nào cấp được (caller báo lỗi).
   */
  async function acquireStream(src: RecordSources): Promise<MediaStream> {
    const audioTracks: MediaStreamTrack[] = [];
    let videoTrack: MediaStreamTrack | null = null;

    if (src.systemAudio || src.screen) {
      const display = await navigator.mediaDevices.getDisplayMedia({
        video: { frameRate: { ideal: 30, max: 30 } },
        audio: src.systemAudio, // chỉ xin audio khi thực sự cần tiếng hệ thống
      });
      const vt = display.getVideoTracks()[0] ?? null;
      if (src.screen && vt) {
        videoTrack = vt;
        vt.addEventListener("ended", () => void stop()); // user bấm "Stop sharing" của browser
      } else {
        vt?.stop(); // chỉ cần tiếng hệ thống → bỏ video track
      }
      const sysTracks = src.systemAudio ? display.getAudioTracks() : [];
      audioTracks.push(...sysTracks);
      // Không giữ video → theo dõi audio track để bắt "Stop sharing".
      if (src.systemAudio && !src.screen) sysTracks[0]?.addEventListener("ended", () => void stop());
      if (!src.systemAudio) display.getAudioTracks().forEach((t) => t.stop());
    }

    if (src.mic) {
      try {
        const micStream = await navigator.mediaDevices.getUserMedia({ audio: true });
        const micTracks = micStream.getAudioTracks();
        // Mic track "ended" bất ngờ = mất nguồn (iOS thu hồi khi khóa màn hình, rút thiết bị) → dừng & lưu.
        for (const t of micTracks) {
          t.addEventListener("ended", () =>
            handleInterrupt("Bản ghi đã dừng (mất micro — có thể do màn hình bị khóa) — đã lưu phần ghi được."),
          );
        }
        audioTracks.push(...micTracks);
      } catch (e) {
        // Mic bị từ chối: nếu là nguồn duy nhất → báo lỗi; còn nguồn khác → bỏ qua mic.
        if (audioTracks.length === 0 && !videoTrack) throw e;
      }
    }

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

  /**
   * Screen Wake Lock giữ màn hình không tự tắt khi đang ghi (iOS 16.4+, Chrome/Edge). Chỉ chống
   * auto-lock — KHÔNG chống user chủ động bấm nút khóa máy. Không hỗ trợ → bỏ qua (ghi vẫn chạy).
   * Wake lock tự nhả khi trang ẩn nên onVisibility xin lại khi trang hiện lại (nếu còn đang ghi).
   */
  async function acquireWakeLock(): Promise<void> {
    try {
      if ("wakeLock" in navigator) wakeLock = await navigator.wakeLock.request("screen");
    } catch {
      wakeLock = null; // trình duyệt từ chối (vd trang không hiển thị) — không sao
    }
  }

  async function releaseWakeLock(): Promise<void> {
    try {
      await wakeLock?.release();
    } catch {
      /* đã nhả / không hỗ trợ */
    }
    wakeLock = null;
  }

  /** Nguồn ghi mất giữa chừng (mic ended do iOS suspend, rút thiết bị) → dừng & lưu phần đã ghi, báo rõ. */
  function handleInterrupt(note: string): void {
    if (!recording.value) return;
    interruptNote = note;
    void stop();
  }

  /**
   * Trang quay lại tiền cảnh. iOS đình chỉ trang khi khóa màn hình / đổi app → thu hồi mic. Nếu mọi
   * audio track đã "ended" trong lúc ẩn → dừng & lưu (event "ended" có thể không kịp fire lúc suspend).
   * Track còn "live" (desktop chỉ đổi tab) → chỉ xin lại wake lock, ghi tiếp — KHÔNG dừng nhầm.
   */
  function onVisibility(): void {
    if (document.visibilityState !== "visible" || !recording.value || !stream) return;
    const audio = stream.getAudioTracks();
    const dead = audio.length > 0 && audio.every((t) => t.readyState === "ended");
    if (dead) handleInterrupt("Bản ghi đã dừng khi màn hình bị khóa / chuyển ứng dụng — đã lưu phần ghi được.");
    else void acquireWakeLock();
  }

  async function start(src: RecordSources): Promise<void> {
    // Reentrancy guard: chặn start khi đang ghi / đang khởi động / đang finalize phiên trước.
    if (recording.value || starting.value || finalizing.value) return;
    starting.value = true;
    error.value = "";
    interruptNote = null;
    chainError = null;
    offset = 0;
    elapsedS.value = 0;
    recordName = `Bản ghi ${new Date().toLocaleString("vi-VN")}`;
    try {
      // Lấy stream NGAY trong transient user-activation của cú bấm: getUserMedia/getDisplayMedia trên
      // WebKit (Safari/Edge/Chrome iOS) chỉ được cấp quyền khi gọi trước mọi await khác — một await fetch
      // xen giữa sẽ mất activation → NotAllowedError. Init session BE sau (cũng tránh tạo phiên rác khi
      // quyền bị từ chối).
      stream = await acquireStream(src);
      if (stream.getTracks().length === 0) {
        throw new Error("Không lấy được nguồn ghi nào — kiểm tra quyền mic / chia sẻ màn hình.");
      }
      const init = await postJson<{ sessionId: string }>("/api/recordings/init", { name: recordName });
      sessionId = init.sessionId;
      const sid = init.sessionId;
      previewStream.value = stream;
      const hasVideo = stream.getVideoTracks().length > 0;
      const options: MediaRecorderOptions = { mimeType: pickMime(hasVideo), audioBitsPerSecond: AUDIO_BPS };
      if (hasVideo) options.videoBitsPerSecond = SCREEN_VIDEO_BPS;
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
      void acquireWakeLock(); // giữ màn hình sáng khi ghi (chống auto-lock)
      document.addEventListener("visibilitychange", onVisibility); // bắt suspend khi khóa/đổi app
    } catch (e) {
      error.value = startErrorMessage(e, src);
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
      // Gián đoạn (khóa màn hình…) ưu tiên báo lý do dừng; nếu không thì cảnh báo lỗi mạng chunk (nếu có).
      if (interruptNote) {
        error.value = interruptNote;
      } else if (chainError) {
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
      interruptNote = null;
    }
  }

  function cleanupStream(): void {
    document.removeEventListener("visibilitychange", onVisibility); // idempotent
    void releaseWakeLock();
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
