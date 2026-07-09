# Memory Index — OneScribe (code-reviewer)

- [transcribe_msg cần case "error" ở mọi composable](onescribe-transcribe-msg-error-pattern.md) — thiếu case error → state treo vĩnh viễn (đặc biệt trong keep-alive)
- [sidecar Python xử lý stdin tuần tự](onescribe-sidecar-sequential-stdin.md) — không race nội bộ Python, race (nếu có) nằm ở Rust/UI
