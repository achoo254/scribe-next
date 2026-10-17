# Code review — free-tier storage, quick delete, delete-all, uploads in history

Commits: d63a737 (server), 40338ec (web), fc3ca96 (docs). Range ec381b6..HEAD, excluding the docs-only commits 72ce548 and deac6ce.
Plan: plans/dattqh/261017-0644-free-tier-storage-and-upload/plan.md
Method: read the diff plus the code around it (retention, search index, transcript_doc orphans, transcribe create/worker, iOS UploadQueue.swift, Radix toast dist, nginx config). No tests were run because the brief says they already pass.

## Verdict

**No Critical findings.** I found no ownership leak, no quota bypass beyond what already existed, and no corruption of the recording offset protocol. Three Medium issues are worth fixing. M1 and M2 can lose user data in edge cases. M3 means one of the guarantees in the brief only holds some of the time. None of them blocks a prod deploy if the owner accepts the risk.

## Explicit checks (requested)

| Check | Result | Evidence |
|---|---|---|
| Delete-all ownership | OK | `files.rs:117` uses `list_history`, which filters on `userId` (`models/file.rs:224`). It deletes only docs from that list (`delete_doc` by `_id`), and the disk path comes from the owned doc. Rate limit is 3/min. Cross-site DELETE is blocked by the SameSite=Lax cookie (`auth.rs:91`). |
| Delete-all spares open sessions | **Partial**, see M3 | Only sessions that have a `recording_sessions` doc right now are spared. |
| Quota race vs before | No regression | Same structure as before (`usage + inflight(.part)`), with the per-plan quota read on each call. The window during finalize where bytes are counted nowhere already existed (see L4). |
| Recording chunk at quota (offset/resume) | OK | `recordings.rs:121-135`: `set_len(current_len)` cuts back to the length before this request. Retries and iOS restart-from-0 resend the same bytes at the same offset, so any overwritten range `[offset, current_len)` gets back identical bytes. iOS maps 413 to `quotaFull`, pauses, and later resends from offset 0 into the same server session (`UploadQueue.swift:192,262`). Web `RecordingChunkQueue` stops on 413 and finalizes. Before this change a 413 deleted the whole `.part`; now only the last chunk is dropped. |
| Undo timers / wrong id after refetch | OK | Ids are captured when the user clicks (`use-undoable-delete.ts` `remove(ids)`), not looked up by index, so a refetch cannot shift them. A 404 counts as deleted. `settled` stops a double send between Action, onClose, flush and unmount. Radix handles `duration: Infinity` correctly (`react-toast/dist/index.mjs:354`). |
| Uploads named `<uuid>#N` merging into iPhone sessions | OK | Server: `session_key` ignores segment names unless kind is `recording` (`models/file.rs:313`). The `s:` branches of `list_session` and `session_heads` filter `kind: "recording"`. Web: `groupSessions` skips `kind === "upload"`. Only remaining gap is L5 (iOS lookup). |
| `expiresAt` per plan / plus lapse | Behaves as designed, but the UI is wrong (M2) | `Owner::expiry` uses the plan when the file is created. When plus lapses, existing files keep 14 days until the admin command runs again. |
| Admin `shorten-free-retention` | OK, idempotent | The filter is `userId $nin plusIds` plus `$expr expiresAt > createdAt + 7d`. A second run matches 0 files. Lapsed plus users (`planUntil` in the past) count as free, as intended. `createdAt` is a required field, so `$add` cannot produce null. Dry run by default. |
| Search index / reconcile with uploads | OK, no rebuild loop | `transcribed_sessions` and `schedule_file` derive the `f:` key from `FileDoc::session_key`. `docs::load_db` reaches uploads through `list_session` (`f:` branch now `kind $in HISTORY_KINDS`), so the rebuilt key equals the reconciled key. `transcript_doc::delete_orphans` also goes through `list_session`, so upload titles, edits and minutes are not swept as orphans. |

## Critical
None.

## High
None.

## Medium

### M1. Plan lapse during a long recording deletes the whole recording at finalize
`server/scribe-be/src/recordings.rs:169-172` (and `upload.rs:177-180`, though an upload can be resent from the local file so the loss there is smaller).

Scenario: a plus user is recording a screen capture that is already about 600 MB. Their `planUntil` passes, or an admin runs `set-plan free`, mid-recording. The next chunk gets `written > 200 MB`, returns 413, and is truncated, so the recorder stops and finalizes. `finalize` re-reads the plan, sees `size > max_file_bytes` (200 MB), runs `remove_file(&part)` and returns 400. **The whole 600 MB recording is lost.** Before this change the cap was a constant, so this could not happen.

Fix: finalize should not re-apply the size cap to bytes the chunk handler already accepted. The chunk path already bounds growth. Either drop the `size > max` check in recordings finalize (keep `size == 0`), or store `maxFileBytes` on the session at init and check against that. Do not delete the `.part` on this path in either case.

### M2. Expiry date shown in History uses the current plan, not the file's real `expiresAt`
`server/scribe-be/src/files.rs:33,52` sends `retentionDays` from the *current* plan. `web/src/pages/record/recording-row.tsx:72` shows `createdAt + retentionDays`.

Scenario: a free user's files have `expiresAt = createdAt + 7d`. They are upgraded to plus. The UI now says "tự xoá sau 14 ngày" for those files, but the sweep deletes them on day 7. The user waits, thinking they have a week left, and the file is gone. (The reverse case after a lapse is harmless: the UI shows 7, the file is kept 14.)

Fix: add `"expiresAt": f.expires_at.try_to_rfc3339_string().ok()` to each item in `list` and `manage_list`, and have the row use it. Keep `retentionDays` only for the header text.

### M3. Delete-all deletes earlier parts of an iPhone recording that is still in progress
`server/scribe-be/src/recordings.rs:312-334`, `files.rs:85-95`.

`active_session_keys` only sees a `recording_sessions` doc. iOS creates that doc per segment, when the segment upload starts (`UploadQueue.swift:252`), and finalize deletes it (`recordings.rs:243-247`). While the phone is in the middle of a 1-minute segment with every earlier segment already uploaded, no session doc exists. Delete-all then removes `#0…#k`. Later segments keep arriving and form a session with missing parts. The UI tells the user that skipped files "belong to a recording still in progress", so they will assume ongoing recordings are safe.

It is the user's own data and they asked to delete everything, so this is not a security issue. It does contradict the "cannot delete files of open sessions" guarantee.

Fix options:
- (a) Also spare `s:` sessions whose newest segment was created within the last few minutes (for example `createdAt > now - 3 × segment length`).
- (b) Have iOS report the live session id (heartbeat) and spare it.
- (c) Document the protection as "only while a segment upload is in flight" and change the UI copy.

Option (a) is cheap and server-only.

## Low

- **L1. Delete-all on a large library can be cut off half-way.** `files.rs:122-133` deletes sequentially, about 5 Mongo ops per file plus a wait on the per-session index lock. A plus user with thousands of iPhone segments can pass Cloudflare's roughly 100 s proxy limit (nginx itself allows 3600 s). The client disconnects, hyper drops the handler, and deletion stops part-way. The user sees an error toast and a partly deleted list. Re-running finishes the job, and the half-deleted state is handled (disk NotFound is OK; orphan sweeps tidy up `transcript_docs` and sentences). Consider running the loop in a `tokio::spawn` so a disconnect cannot cancel it, or deleting in bulk.
- **L2. Undo commit is not covered by `keepalive`.** `use-undoable-delete.ts` `commit()` sends DELETEs one at a time with `keepalive: false`, and it removes the entry from `pending` before the loop. Deleting a 120-part session and closing the tab 1–2 s after the 5 s window leaves the session partly deleted. `flush` cannot help because the entry is already gone from `pending`. Fix: use `keepalive: true` in commit too (DELETEs have no body, so the 64 KB keepalive quota is not a concern).
- **L3. `toggling` ref can stay set, so the History list is not refreshed.** `record-state.tsx:143-167`. If the user presses Stop while `onFull` has already started `recorder.stop()`, the user's `stop()` returns null, `toggle` returns early, and the effect then clears `toggling` instead of refreshing. The saved recording does not appear until the next refetch. Fix: set `toggling.current = true` only after `stop()` returns a result, or reset it on the null path. UI only.
- **L4. Quota window during finalize (already existed).** `upload.rs` finalize renames, then probes, then `file::create`. Recordings remove the `.part` after remux, then probe, then create. During that gap the bytes are counted neither as inflight nor as usage. A concurrent chunk of another upload (body up to the edge limit of about 100 MB) can pass the check, so a free user can briefly go over 200 MB by at most about one file. Not new, and bounded. Mention it only because 200 MB makes the overrun proportionally larger.
- **L5. iOS `lookUp(name)` now also matches uploads.** `UploadQueue.swift:293-298` matches by name with no kind filter, and GET `/api/files` now includes uploads. An upload would need to carry the same random `<uuid>#N` name, which is practically impossible. Filtering `kind != "upload"` in a later iOS build is enough.

## Positive observations (for risk calibration)
- `split_for_delete_all` is a pure function with a unit test that covers the upload-named-like-a-segment case.
- The 413 path in `RecordingChunkQueue` has no retry and sends nothing after the refused chunk. A test covers it.
- Every `session_key` call site (`grep session_key(`) was updated to pass the kind. The only hardcoded `"recording"` calls are for files that really are recordings (`recordings.rs:239`, `jobs.rs:318`).

## Recommended actions (priority)
1. M1: stop deleting the `.part` and drop the plan-size recheck in recordings finalize. This is small and prevents a whole recording being lost.
2. M2: return per-file `expiresAt` and show it in the rows.
3. M3: spare `s:` sessions with a very recent segment, or change the guarantee and the UI copy.
4. L2: use `keepalive` in `commit`.
5. L1, L3, L5 when convenient.

## Plan status
Phase 1 (backend + web + test) looks complete against the acceptance criteria. Phase 2 (deploy + running `shorten-free-retention --apply` after a backup) is still pending. The command is safe to dry-run first and is idempotent.

## Unresolved questions
- Is the M3 behaviour acceptable to the owner (Jev), or is a hard guarantee for open iPhone sessions required?
- When a plus plan lapses, should `shorten-free-retention` run automatically (for example from the sweep), or stay manual?

Status: DONE_WITH_CONCERNS
Summary: No Critical or High defects. Ownership, quota, the offset protocol, undo id capture, upload/segment key separation, the admin command and index reconcile all hold up. Three Medium issues: a whole recording is deleted at finalize after a mid-recording plan downgrade, the expiry date shown in the UI uses the current plan, and delete-all does not fully protect live iPhone sessions.
Concerns: M1 and M2 can lose user data in edge cases. M3 means one of the guarantees in the brief only holds some of the time.
