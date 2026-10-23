# B-fixes: backend review findings (done 2026-10-23)

Fixes for `docs/progress/review-B0-B5.md`, plus two findings from the wiring test (`W.md`). Only `backend/` was
changed. No JSON field names or route paths changed.

## For the frontend: new error names
The frontend error lists (`types/errors.ts`, `api/errors.ts`) should know these. Without them, `client.ts` maps
them to `Internal`.

| Name | Code | HTTP | When |
|---|---|---|---|
| `RequestInvalid` | 1000 | 400 | A bad query or body (already sent since B4, still missing in the frontend). |
| `RouteNotFound` | 1006 | 404 | Unknown path. The body is now `{ error: { name, code, message } }`, not empty. |
| `MethodNotAllowed` | 1007 | 405 | Known path, wrong method. Same JSON shape. |
| `RequestCancelled` | 1008 | 499 | Only sent when the client has already disconnected, so the browser never sees it. Listed for completeness. |

Spec §8 (outside `backend/`) should get these 4 rows too.

## Findings
| # | Status | What changed |
|---|---|---|
| H1 outline recursion | fixed | `pdfium_text_source.rs`: walks the outline with an explicit stack. It skips bookmarks it has already seen (`HashSet<PdfBookmark>`, so cycles through `/First` or `/Next` are cut), drops levels below `MAX_OUTLINE_DEPTH = 16`, and keeps the 20 000-item cap. The order (pre-order) is unchanged. |
| M1 stale `Now` jobs | fixed (option 1: cancel on drop) | `RequestContext` has a `Cancellation` flag. The audio controller holds a `CancelOnDrop` guard, which axum drops along with the handler future when the client disconnects. `SpeakSentence` waits with `SpeechTicket::wait_unless_cancelled` (checks every 100 ms). When the flag is set, it withdraws its waiter and returns `RequestCancelled`. The worker drops a queued job once it has no waiters left. Also, when one waiter is withdrawn, a merged job falls to the highest priority its remaining waiters asked for, so a cancelled `Now` no longer keeps a read-ahead job at `Now`. I picked this over demoting older jobs because it also covers jumping back, different voices and rates, and any client that gives up, without guessing which jobs are stale. **Checked on the real server** (debug build, DDS, scratch data dir): 6 requests (900–902, 1500–1502) were aborted after 0.3 s, then 2100 was requested. The log shows 6 "speech job withdrawn" lines. Only 900 (already running) and 2100 were synthesized, and 2100 took 2.9 s in all instead of waiting behind 5 more jobs. |
| L1 zero or tiny font size | fixed | `cleaning/code.rs`: the char width is never below `MIN_CHAR_WIDTH = 1.0` pt (a NaN size also ends up at 1.0). Each indent or gap adds at most `MAX_SPACES = 80` spaces, and a NaN gap adds none. **Fuzzed again**: 20 000 random documents with 25 % zero, tiny, NaN or negative sizes (16 216 of them had code blocks) gave 0 panics, and 20 000 with realistic sizes gave 0 panics. The harness is in the session scratchpad, not in the repo. |
| L2 `FsAudioCache::get` race | fixed | On `NotFound`, the entry is removed only if the file is still missing while the lock is held. |
| L3 keep window = read-ahead | fixed | `ReadAheadSettings::keeps_enough()`: keep ≥ 1.5 × read-ahead (`MIN_KEEP_TO_READ_AHEAD_RATIO`). The config uses it (`audio.keep_minutes_per_document` is rejected otherwise), and so does `ReadAhead::new`, which now returns `AppResult` (`RequestInvalid`). The shipped 10 / 60 is fine. |
| L4 / A6 service-level validation | fixed | `SpeakSentence::run` rejects a rate that isn't finite or is ≤ 0 (`RequestInvalid`). `ListRecentDocuments` checks the limit itself (`ListOptions::has_valid_limit`, 1–`MAX_LIMIT`), and the route uses the same check. Also, the cache window now deletes nothing for a NaN `keep_seconds` instead of everything. I didn't make the rate a domain type: it would have touched every speech struct for little gain now that the service checks it. |
| L5 re-clean overwrites a position | fixed | `DocumentMeta::merged_with_saved` (domain): when a document is saved again, the stored position wins if it still names a sentence, and the earliest add and latest open are kept. `FsDocumentRepository::save` merges under `meta_lock`, and the fake does the same. The behavior is documented on `DocumentWriter::save`. |
| L6 hard-coded 24 kHz WAV | fixed | The cache index stores each clip's `audio_seconds`, read from its WAV header (in `put` and in the startup scan, header only). `WAV_HEADER_BYTES` and `WAV_BYTES_PER_SECOND` are gone. The silence clip uses the new `SpeechEngine::sample_rate()`. B6's export already took the rate from the audio. |
| L7 callback panics | fixed | `deliver` wraps each callback in `catch_unwind` and logs a `warn`. The comment is fixed (the *last* waiter gets the original error). |
| A1 `sha2` in app | fixed | New port `ContentHasher`, infra adapter `Sha256ContentHasher` (`text/content_hasher.rs`), injected into `ImportPdf`. `sha2` moved to infra. App tests use a `FakeHasher`. |
| A2 duplication | fixed | `Document::sentence_count()` in the domain (used by app, infra and `dev_clean`). One `SECONDS_PER_MINUTE`. One `storage_failed` in `storage/mod.rs` (it replaces the 4 copies, including B6's `export_records.rs` and `ffmpeg.rs`). |
| A3 magic `20` | fixed | `ListOptions::DEFAULT_LIMIT` / `MAX_LIMIT` / `limit_or_default()`, used by the repository, the fake and the route. |
| A4 tower-http `limit` | fixed | Feature removed. |
| A5 `ReadAheadPlan::ids()` | fixed | Now `#[cfg(test)]` and private. |
| Extra: `CachedLibrary::find` deep clone | not fixed | Returning `Arc<Document>` changes the `DocumentReader` port and every caller. The review listed it as "worth a look", not as a finding, so I left it for later. |
| Extra: noisy per-request logs | fixed | "document loaded", "audio cache looked up", "audio encoded" and "audio cached" are now `debug`. |
| Contract 3: 404/405 shape | fixed | `Router::fallback` → `RouteNotFound`, `method_not_allowed_fallback` → `MethodNotAllowed`. Both checked on the real server too (92-byte JSON 404, JSON 405). |
| Contract 1, 2 | frontend | `speechText` skipping and `RequestInvalid` belong to the frontend (W step). |
| `[export] max_sentences` | checked | `config.toml` has `max_sentences = 0`, which composition maps to `None` (the whole book). A config test asserts the shipped value. |
| W 1: export time left too low | fixed | `exports/progress.rs` `SynthesisRate`: the rate is the wall time between pipeline outputs, counted for **synthesized sentences only**, over their characters (cumulative, so it's smoothed). Cached and silent sentences finish at once and no longer count. The estimate is `null` until one sentence was synthesized, and `0` when done. Remaining cached sentences can make it a little high, never low. |
| W 2: M1 on the real stack | fixed | See M1 above. |

## Tests added (284 pass, 11 ignored; `--ignored` passes too)
- infra `tests/pdf_outline.rs` (builds tiny PDFs in the test): a bookmark that is its own `/First`, one that is its own
  `/Next`, and a 5 000-deep chain read on a 256 KiB thread stack (cut to 16 levels).
- domain `code.rs`: zero, tiny and `MIN_POSITIVE` sizes give bounded spaces; NaN and infinite gaps.
- infra cache: a NaN window deletes nothing; length from the header at 22.05, 24 and 44.1 kHz; a 44.1 kHz window
  measured in seconds (also after reopening); a `get` miss racing a `put` keeps the clip (300 rounds). The existing
  cache tests now use real WAV clips.
- app worker: a cancelled `Now` waiter lowers a merged job to read-ahead; a cancelled caller's job is dropped
  (`RequestCancelled`); a panicking callback leaves the worker running.
- app `SpeakSentence`: a cancelled request withdraws its job (never synthesized); a NaN, infinite, 0 or negative rate
  gives `RequestInvalid` before anything runs.
- app `ListRecentDocuments`: limit 0 and 101 are rejected. App exports: time left is `null` while only cached sentences are done.
  Progress: the rate math, and cached sentences not speeding it up.
- infra repository: a position and open saved during a re-save are kept; a position the new document lacks is dropped.
- api: unknown paths → `RouteNotFound` 404 JSON; a wrong method → `MethodNotAllowed` 405 JSON; dropping the handler
  future cancels the request; config rejects keep = read-ahead and accepts 15 / 10.
- domain error registry count is now 20.

## Notes
- `pdf_support::text_source()` is shared through a `OnceLock`, because PDFium can only be bound once per process and
  tests run in parallel.
- `cargo test -- --ignored` also runs `write_cleaner_fixtures`, which rewrites `lectrice-domain/tests/fixtures/*.jsonl`
  from the books. The outline order is unchanged, and the domain fixture tests still pass afterwards.
- `cargo build`, `cargo test`, `cargo test -- --ignored`, `cargo clippy --all-targets -- -D warnings` and
  `cargo fmt --check` all pass.
