// G6 mini-spike (Phase 1 gate): đo CHÂN audio WebView2 → Rust. Unknown khó nhất: getUserMedia →
// AudioWorklet bắt PCM có chạy trong WebView2 không + ship JS→Rust qua IPC binary đủ throughput/latency
// không. (Latency ASR đã đo G1/G2.) Đo round-trip JS→Rust→JS + throughput + kiểm COOP/COEP cho SAB.
import { ref, onUnmounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

interface Echo {
  seq: number;
  t0_ms: number;
  bytes: number;
}

export function useAudioTapProbe() {
  const running = ref(false);
  const error = ref("");
  const actualSampleRate = ref(0);
  const sabSupported = ref(typeof SharedArrayBuffer !== "undefined");
  const crossOriginIsolated = ref(self.crossOriginIsolated === true);
  const chunksSent = ref(0);
  const bytesSent = ref(0);
  const rttP50 = ref(0);
  const rttP95 = ref(0);
  const rttLast = ref(0);

  let ctx: AudioContext | null = null;
  let stream: MediaStream | null = null;
  let node: AudioWorkletNode | null = null;
  let source: MediaStreamAudioSourceNode | null = null;
  let unlisten: UnlistenFn | null = null;
  let seq = 0;
  let pending = new Float32Array(0);
  const CHUNK = 16000; // ~1s @16kHz (stream_pcm schema)
  const rtts: number[] = [];

  function percentile(arr: number[], p: number): number {
    if (!arr.length) return 0;
    const s = [...arr].sort((a, b) => a - b);
    return Math.round(s[Math.min(s.length - 1, Math.floor((p / 100) * s.length))] * 100) / 100;
  }

  function toInt16(f32: Float32Array): Int16Array {
    const out = new Int16Array(f32.length);
    for (let i = 0; i < f32.length; i++) {
      const v = Math.max(-1, Math.min(1, f32[i]));
      out[i] = v < 0 ? v * 32768 : v * 32767;
    }
    return out;
  }

  async function flushChunk() {
    if (pending.length < CHUNK) return;
    const slice = pending.subarray(0, CHUNK);
    pending = pending.slice(CHUNK);
    const pcm = new Uint8Array(toInt16(slice).buffer);
    const mySeq = seq++;
    const t0 = performance.now();
    chunksSent.value += 1;
    bytesSent.value += pcm.byteLength;
    // Tauri v2: Uint8Array arg được truyền raw (không JSON-array) — chính điểm cần verify throughput.
    invoke("meeting_pcm_probe", { seq: mySeq, t0Ms: t0, pcm }).catch((e) => {
      error.value = `invoke lỗi: ${e}`;
    });
  }

  async function start() {
    if (running.value) return;
    error.value = "";
    chunksSent.value = 0;
    bytesSent.value = 0;
    rtts.length = 0;
    try {
      unlisten = await listen<Echo>("meeting_pcm_echo", (e) => {
        const rtt = performance.now() - e.payload.t0_ms;
        rttLast.value = Math.round(rtt * 100) / 100;
        rtts.push(rtt);
        rttP50.value = percentile(rtts, 50);
        rttP95.value = percentile(rtts, 95);
      });
      stream = await navigator.mediaDevices.getUserMedia({ audio: true, video: false });
      // Ép 16kHz để khỏi resample; nếu WebView2 không honor, ghi nhận actualSampleRate.
      ctx = new AudioContext({ sampleRate: 16000 });
      actualSampleRate.value = ctx.sampleRate;
      await ctx.audioWorklet.addModule("/pcm-tap-worklet.js");
      source = ctx.createMediaStreamSource(stream);
      node = new AudioWorkletNode(ctx, "pcm-tap");
      node.port.onmessage = (ev) => {
        const f = ev.data as Float32Array;
        const merged = new Float32Array(pending.length + f.length);
        merged.set(pending, 0);
        merged.set(f, pending.length);
        pending = merged;
        void flushChunk();
      };
      source.connect(node);
      // KHÔNG connect node→destination (tránh nghe lại tiếng mình); worklet vẫn chạy nhờ return true.
      running.value = true;
    } catch (e) {
      error.value = String(e);
      await stop();
    }
  }

  async function stop() {
    running.value = false;
    try {
      node?.port && (node.port.onmessage = null);
      source?.disconnect();
      node?.disconnect();
      stream?.getTracks().forEach((t) => t.stop());
      await ctx?.close();
    } catch {
      /* ignore cleanup error */
    }
    ctx = null;
    stream = null;
    node = null;
    source = null;
    pending = new Float32Array(0);
    if (unlisten) {
      unlisten();
      unlisten = null;
    }
  }

  onUnmounted(stop);

  return {
    running, error, actualSampleRate, sabSupported, crossOriginIsolated,
    chunksSent, bytesSent, rttP50, rttP95, rttLast, start, stop,
  };
}
