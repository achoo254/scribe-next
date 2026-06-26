"""Đo RSS (RAM), CPU% và tổng hợp latency/RTF cho spike.

RSS: psutil nếu có; nếu không → ctypes Win32 GetProcessMemoryInfo (zero-dep trên Windows).
CPU%: chỉ khi có psutil (fallback ghi N/A — latency + RTF mới là số quyết định GO/NO-GO).
"""
import threading
import time

try:
    import psutil  # tùy chọn

    _PROC = psutil.Process()
    _HAS_PSUTIL = True
except Exception:
    _PROC = None
    _HAS_PSUTIL = False


def _win_rss_mb() -> float:
    """RSS (WorkingSetSize) qua Win32 psapi — fallback khi không có psutil."""
    import ctypes
    import ctypes.wintypes as wt

    class _PMC(ctypes.Structure):
        _fields_ = [
            ("cb", wt.DWORD),
            ("PageFaultCount", wt.DWORD),
            ("PeakWorkingSetSize", ctypes.c_size_t),
            ("WorkingSetSize", ctypes.c_size_t),
            ("QuotaPeakPagedPoolUsage", ctypes.c_size_t),
            ("QuotaPagedPoolUsage", ctypes.c_size_t),
            ("QuotaPeakNonPagedPoolUsage", ctypes.c_size_t),
            ("QuotaNonPagedPoolUsage", ctypes.c_size_t),
            ("PagefileUsage", ctypes.c_size_t),
            ("PeakPagefileUsage", ctypes.c_size_t),
        ]

    k32 = ctypes.WinDLL("kernel32", use_last_error=True)
    psapi = ctypes.WinDLL("psapi", use_last_error=True)
    k32.GetCurrentProcess.restype = wt.HANDLE
    psapi.GetProcessMemoryInfo.argtypes = [wt.HANDLE, ctypes.POINTER(_PMC), wt.DWORD]
    psapi.GetProcessMemoryInfo.restype = wt.BOOL
    pmc = _PMC()
    pmc.cb = ctypes.sizeof(_PMC)
    if psapi.GetProcessMemoryInfo(k32.GetCurrentProcess(), ctypes.byref(pmc), pmc.cb):
        return pmc.WorkingSetSize / 1e6
    return 0.0


def rss_mb() -> float:
    if _HAS_PSUTIL:
        return _PROC.memory_info().rss / 1e6
    try:
        return _win_rss_mb()
    except Exception:
        return 0.0


class ResourceSampler:
    """Thread nền: lấy mẫu RSS + CPU% mỗi `interval_s`, giữ peak RSS và CPU trung bình."""

    def __init__(self, interval_s: float = 0.2):
        self.interval_s = interval_s
        self.peak_rss = rss_mb()
        self._cpu_samples = []
        self._stop = threading.Event()
        self._t = None
        if _HAS_PSUTIL:
            _PROC.cpu_percent(None)  # mồi cho lần đọc sau

    def _loop(self):
        while not self._stop.is_set():
            self.peak_rss = max(self.peak_rss, rss_mb())
            if _HAS_PSUTIL:
                self._cpu_samples.append(_PROC.cpu_percent(None))
            self._stop.wait(self.interval_s)

    def start(self):
        self._t = threading.Thread(target=self._loop, daemon=True)
        self._t.start()
        return self

    def stop(self):
        self._stop.set()
        if self._t:
            self._t.join(timeout=1.0)
        self.peak_rss = max(self.peak_rss, rss_mb())

    @property
    def avg_cpu(self):
        # Bỏ mẫu đầu: nó đo CPU% từ lúc prime (__init__) tới tick đầu → dính thời gian warm-up.
        s = self._cpu_samples[1:] if len(self._cpu_samples) > 1 else self._cpu_samples
        if not s:
            return None
        return sum(s) / len(s)


def percentile(values, p: float) -> float:
    if not values:
        return 0.0
    s = sorted(values)
    k = (len(s) - 1) * (p / 100.0)
    lo = int(k)
    hi = min(lo + 1, len(s) - 1)
    return s[lo] + (s[hi] - s[lo]) * (k - lo)


class Stats:
    """Gom latency (final caption) + RTF (mỗi lần gọi transcribe) để tính p50/p95."""

    def __init__(self):
        self.latencies = []  # giây: caption final xuất hiện sau khi câu nói xong
        self.rtfs = []       # compute_time / audio_dur của từng lần transcribe
        self.max_lag = 0.0   # realtime: feeder tụt sau wall-clock bao nhiêu (s) — latency chỉ tin khi nhỏ
        self.lock = threading.Lock()

    def add_latency(self, sec: float):
        with self.lock:
            self.latencies.append(sec)

    def note_lag(self, lag_s: float):
        with self.lock:
            if lag_s > self.max_lag:
                self.max_lag = lag_s

    def add_rtf(self, compute_s: float, audio_dur_s: float):
        if audio_dur_s <= 0:
            return
        with self.lock:
            self.rtfs.append(compute_s / audio_dur_s)

    def summary(self) -> dict:
        return {
            "n_finals": len(self.latencies),
            "latency_p50_s": round(percentile(self.latencies, 50), 3),
            "latency_p95_s": round(percentile(self.latencies, 95), 3),
            "latency_max_s": round(max(self.latencies), 3) if self.latencies else 0.0,
            "n_transcribe_calls": len(self.rtfs),
            "rtf_mean": round(sum(self.rtfs) / len(self.rtfs), 3) if self.rtfs else 0.0,
            "rtf_p95": round(percentile(self.rtfs, 95), 3),
            "rtf_max": round(max(self.rtfs), 3) if self.rtfs else 0.0,
        }


def has_psutil() -> bool:
    return _HAS_PSUTIL


def _now() -> float:
    return time.perf_counter()
