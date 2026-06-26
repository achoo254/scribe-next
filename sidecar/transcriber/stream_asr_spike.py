"""Spike đo streaming ASR (Phase 1 — RISK GATE). THROWAWAY: đo số, KHÔNG phải production.

Mục tiêu: chứng minh máy host transcribe được nhiều track audio kiểu streaming ở latency chấp
nhận (caption final <= ~4s, RTF < 1) trên CPU yếu — quyết định GO/NO-GO trước khi xây platform.

Chạy (dev, từ thư mục sidecar/transcriber):
  .venv\\Scripts\\python.exe stream_asr_spike.py \\
      --model-dir "<app_data>/vn.inet.onescribe/models/small/PhoWhisper-small-ct2-fasterWhisper" \\
      --wav speaker_a.wav --wav speaker_b.wav \\
      --cpu-threads 4 --strategy local-agreement --realtime --out spike_run.json

  # Đo RAM small+medium đồng thời (kiểm OOM trên 8GB) [C1]:
  .venv\\Scripts\\python.exe stream_asr_spike.py --probe-ram \\
      --model-dir "<...>/small/..." --medium-dir "<...>/medium/PhoWhisper-medium-ct2-fasterWhisper"

Caption in ra stdout dạng JSON-line: {"type":"caption","track","kind","text","t0","t1","latency_s"}.
Tổng kết (latency p50/p95, RTF, peak RAM, CPU) in cuối + ghi --out (JSON) nếu có.
"""
import argparse
import json
import sys
import threading
import time

import numpy as np

from spike_lib import audio, engine, metrics, vad

_STDOUT_LOCK = threading.Lock()  # tránh interleave dòng JSON giữa các thread track


def emit_line(obj):
    line = json.dumps(obj, ensure_ascii=False) + "\n"
    with _STDOUT_LOCK:
        try:
            sys.stdout.write(line)
            sys.stdout.flush()
        except UnicodeEncodeError:  # console không utf-8 → không cho 1 dòng làm chết thread track
            sys.stdout.buffer.write(line.encode("utf-8", "backslashreplace"))
            sys.stdout.buffer.flush()


def parse_args():
    p = argparse.ArgumentParser(description="Spike streaming ASR PhoWhisper-small CPU int8")
    p.add_argument("--model-dir", required=True, help="thư mục chứa model.bin (small) cho live")
    p.add_argument("--size", default="small", help="nhãn size (chỉ để báo cáo)")
    p.add_argument("--wav", action="append", default=[], help="1 file = 1 track; lặp lại cho nhiều track")
    p.add_argument("--cpu-threads", type=int, default=0, help="0 = os.cpu_count(); đặt 4 để mô phỏng i5")
    p.add_argument("--strategy", choices=["vad-cut", "local-agreement"], default="vad-cut")
    p.add_argument("--chunk-ms", type=int, default=1000)
    p.add_argument("--hangover-ms", type=int, default=600, help="im lặng tối thiểu để chốt câu (vad-cut)")
    p.add_argument("--commit-ms", type=int, default=1000, help="nhịp chạy lại ASR (local-agreement)")
    p.add_argument("--beam", type=int, default=1)
    p.add_argument("--vad", choices=["energy", "webrtc", "none"], default="energy")
    p.add_argument("--vad-aggr", type=int, default=2)
    p.add_argument("--vad-threshold-dbfs", type=float, default=-45.0)
    p.add_argument("--realtime", action="store_true", help="feed audio đúng nhịp wall-clock (đo latency thật)")
    p.add_argument("--isolated-models", action="store_true", help="mỗi track 1 model riêng (đo RAM/parallel)")
    p.add_argument("--probe-ram", action="store_true", help="nạp small+medium đồng thời, in peak RAM rồi thoát")
    p.add_argument("--medium-dir", default=None, help="thư mục model medium cho --probe-ram")
    p.add_argument("--out", default=None, help="ghi báo cáo JSON ra file")
    return p.parse_args()


def cpu_threads(args) -> int:
    import os
    return args.cpu_threads if args.cpu_threads > 0 else (os.cpu_count() or 8)


def probe_ram(args):
    """Nạp small + medium đồng thời, mỗi model warm 1 lần, đo peak RSS — kiểm OOM 8GB [C1]."""
    sampler = metrics.ResourceSampler().start()
    lock = threading.Lock()
    emit_line({"type": "info", "msg": "probe-ram: nạp small..."})
    small = engine.load_model(args.model_dir, cpu_threads(args))
    warm = np.zeros(engine.SR, np.float32)
    engine.transcribe_words(small, warm, lock, 1, engine.DEFAULT_PROMPT)
    rss_small = metrics.rss_mb()
    if not args.medium_dir:
        sampler.stop()
        emit_line({"type": "error", "msg": "thiếu --medium-dir để đo đồng thời"})
        return {"peak_rss_mb": round(sampler.peak_rss, 1), "rss_after_small_mb": round(rss_small, 1)}
    emit_line({"type": "info", "msg": "probe-ram: nạp medium (giữ small)..."})
    medium = engine.load_model(args.medium_dir, cpu_threads(args))
    engine.transcribe_words(medium, warm, lock, 1, engine.DEFAULT_PROMPT)
    sampler.stop()
    result = {
        "rss_after_small_mb": round(rss_small, 1),
        "peak_rss_both_mb": round(sampler.peak_rss, 1),
        "oom_risk_8gb": sampler.peak_rss > 6500,  # chừa headroom OS + app
        "psutil": metrics.has_psutil(),
    }
    emit_line({"type": "probe_ram", **result})
    _ = (small, medium)  # giữ ref tới khi đo xong
    return result


def make_strategy(args, model, lock):
    if args.strategy == "local-agreement":
        return engine.LocalAgreementStrategy(model, lock, args.beam, engine.DEFAULT_PROMPT,
                                             commit_ms=args.commit_ms)
    return engine.VadCutStrategy(model, lock, args.beam, engine.DEFAULT_PROMPT)


def run_track(track_id, samples, args, model, lock, stats, barrier):
    v, _name = vad.make_vad(args.vad, args.vad_aggr, args.vad_threshold_dbfs)
    strat = make_strategy(args, model, lock)
    hangover = max(1, round(args.hangover_ms / args.chunk_ms))
    barrier.wait()
    t_start = time.perf_counter()

    def emit(caps, runs):
        for r in runs:
            stats.add_rtf(r["compute"], r["audio_dur"])
        for c in caps:
            line = {"type": "caption", "track": track_id, "kind": c["kind"],
                    "text": c["text"], "t0": round(c["t0"], 2), "t1": round(c["t1"], 2)}
            if c["kind"] == "final":
                lat = time.perf_counter() - (t_start + c["t1"])
                line["latency_s"] = round(lat, 3)
                stats.add_latency(lat)
            emit_line(line)

    for pos, end, t0, t1 in audio.chunk_indices(len(samples), args.chunk_ms):
        chunk = samples[pos:end]
        if args.realtime:
            dt = (t_start + t1) - time.perf_counter()
            if dt > 0:
                time.sleep(dt)
            else:
                stats.note_lag(-dt)  # feeder tụt sau wall-clock → latency có thể lạc quan
        emit(*strat.feed(chunk, t0, t1, v.is_speech(chunk), hangover))
    emit(*strat.flush(len(samples) / audio.SR))


def run_tracks(args):
    if not args.wav:
        emit_line({"type": "error", "msg": "cần ít nhất 1 --wav"})
        return None
    ff = audio.find_ffmpeg()
    tracks = [audio.load_pcm_16k_mono(w, ff) for w in args.wav]
    tracks = [(i, s) for i, s in enumerate(tracks) if len(s)]
    if not tracks:
        emit_line({"type": "error", "msg": "không nạp được audio nào"})
        return None

    nthreads = cpu_threads(args)
    lock = threading.Lock()
    if args.isolated_models:
        per_model_threads = max(1, nthreads // len(tracks))  # mỗi model 1 phần lõi
        models = {i: engine.load_model(args.model_dir, per_model_threads) for i, _ in tracks}
        locks = {i: threading.Lock() for i, _ in tracks}
    else:
        per_model_threads = nthreads  # 1 model dùng chung toàn bộ lõi, serialize qua lock
        shared = engine.load_model(args.model_dir, nthreads)
        models = {i: shared for i, _ in tracks}
        locks = {i: lock for i, _ in tracks}

    # warm-up (không tính vào số đo)
    engine.transcribe_words(models[tracks[0][0]], np.zeros(audio.SR, np.float32),
                            locks[tracks[0][0]], args.beam, engine.DEFAULT_PROMPT)

    stats = metrics.Stats()
    sampler = metrics.ResourceSampler().start()
    barrier = threading.Barrier(len(tracks))
    threads = [threading.Thread(target=run_track,
                                args=(i, s, args, models[i], locks[i], stats, barrier))
               for i, s in tracks]
    wall0 = time.perf_counter()
    for t in threads:
        t.start()
    for t in threads:
        t.join()
    wall = time.perf_counter() - wall0
    sampler.stop()

    total_audio = sum(len(s) for _, s in tracks) / audio.SR
    # latency realtime chỉ đáng tin khi feeder không tụt quá ~1 chunk sau wall-clock (H3).
    lag_limit = args.chunk_ms / 1000.0
    latency_valid = (not args.realtime) or stats.max_lag <= lag_limit
    summary = {
        "config": {
            "size": args.size, "strategy": args.strategy, "vad": args.vad,
            "cpu_threads": nthreads, "cpu_threads_per_model": per_model_threads,
            "chunk_ms": args.chunk_ms, "beam": args.beam,
            "n_tracks": len(tracks), "realtime": args.realtime,
            "isolated_models": args.isolated_models, "psutil": metrics.has_psutil(),
        },
        "wall_s": round(wall, 2),
        "total_audio_s": round(total_audio, 2),
        "aggregate_rtf_wall": round(wall / total_audio, 3) if total_audio else None,
        "peak_rss_mb": round(sampler.peak_rss, 1),
        "avg_cpu_pct": round(sampler.avg_cpu, 1) if sampler.avg_cpu is not None else None,
        "realtime_lag_max_s": round(stats.max_lag, 3),
        "latency_valid": latency_valid,
        **stats.summary(),
    }
    if not latency_valid:
        emit_line({"type": "warn", "msg": f"feeder tụt {stats.max_lag:.1f}s sau realtime "
                   f"(>{lag_limit:.1f}s) → latency_p* LẠC QUAN, không tin cho gate G2"})
    emit_line({"type": "summary", **summary})
    return summary


def main():
    try:  # Windows mặc định cp1252 — caption tiếng Việt cần utf-8 (giống main.py)
        sys.stdout.reconfigure(encoding="utf-8")
        sys.stderr.reconfigure(encoding="utf-8")
    except Exception:
        pass
    args = parse_args()
    result = probe_ram(args) if args.probe_ram else run_tracks(args)
    if args.out and result is not None:
        with open(args.out, "w", encoding="utf-8") as f:
            json.dump(result, f, ensure_ascii=False, indent=2)
        emit_line({"type": "info", "msg": f"đã ghi {args.out}"})


if __name__ == "__main__":
    main()
