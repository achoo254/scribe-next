// AudioWorklet (G6 spike): tap PCM từ MediaStream trong WebView2. Gom mono Float32 ~100ms rồi
// postMessage về main thread (transfer ArrayBuffer, zero-copy). Main thread → int16 → invoke Rust.
// AudioContext nên tạo với sampleRate=16000 để khỏi resample (khớp stream_pcm 16kHz mono).
class PcmTapProcessor extends AudioWorkletProcessor {
  constructor() {
    super();
    this._buf = [];
    this._count = 0;
    this._target = 1600; // ~100ms @16kHz
  }

  process(inputs) {
    const input = inputs[0];
    if (input && input[0]) {
      const ch = input[0]; // mono channel 0
      this._buf.push(new Float32Array(ch));
      this._count += ch.length;
      if (this._count >= this._target) {
        const merged = new Float32Array(this._count);
        let off = 0;
        for (const b of this._buf) {
          merged.set(b, off);
          off += b.length;
        }
        this._buf = [];
        this._count = 0;
        this.port.postMessage(merged, [merged.buffer]); // transfer → zero-copy
      }
    }
    return true; // giữ worklet sống
  }
}

registerProcessor("pcm-tap", PcmTapProcessor);
