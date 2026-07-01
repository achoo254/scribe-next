// AudioWorklet tap (Phase 3): gom mẫu audio 16kHz mono float32 → chunk ~chunkMs → post Int16Array
// (little-endian raw) về main thread. Main invoke Rust → sidecar streaming ASR.
//
// AudioContext được tạo ở sampleRate 16000 → track gốc (thường 48kHz) đã được graph resample về 16k
// TRƯỚC khi tới worklet. `sampleRate` là biến global của AudioWorkletGlobalScope = tần số context.
//
// Nạp từ /meeting-audio-tap-worklet.js (public/) — same-origin → hợp CSP `default-src 'self'`,
// KHÔNG cần blob: (blob worklet sẽ vướng CSP).
class MeetingAudioTap extends AudioWorkletProcessor {
  constructor(options) {
    super();
    const chunkMs = (options.processorOptions && options.processorOptions.chunkMs) || 500;
    this._size = Math.max(1, Math.round((sampleRate * chunkMs) / 1000));
    this._buf = new Float32Array(this._size);
    this._n = 0;
  }

  process(inputs) {
    const input = inputs[0];
    const ch = input && input[0];
    if (!ch) return true; // track im/ngắt tạm → giữ processor sống
    for (let i = 0; i < ch.length; i++) {
      this._buf[this._n++] = ch[i];
      if (this._n >= this._size) this._flush();
    }
    return true;
  }

  _flush() {
    const out = new Int16Array(this._n);
    for (let i = 0; i < this._n; i++) {
      const s = Math.max(-1, Math.min(1, this._buf[i]));
      out[i] = s < 0 ? s * 0x8000 : s * 0x7fff;
    }
    // transfer buffer → zero-copy sang main thread
    this.port.postMessage(out.buffer, [out.buffer]);
    this._n = 0;
  }
}

registerProcessor("meeting-audio-tap", MeetingAudioTap);
