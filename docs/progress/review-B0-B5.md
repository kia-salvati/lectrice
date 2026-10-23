# Review of the backend, steps B0–B5 (2026-10-23)

Read-only review of `backend/crates/*` against the design spec (§3–§11), `rules/BACKEND_RULES.md` §1–§7
and the B0–B5 progress notes. Export code (B6) was skipped because it is still being written.

**How the findings were checked:** `cargo test --workspace` passes and `cargo clippy --workspace --all-targets`
has no warnings. I reproduced the High finding against the real release server. L1 came from a small fuzz
harness that runs the real cleaner and pipeline on random `PdfContent` (20 000 cases, kept in the session
scratchpad, not in the repo). Everything else comes from reading the code. Line numbers are from
2026-10-23. The B6 agent is editing `audio_cache.rs`, so its line numbers may move by a line or two.

Counts: **1 High, 1 Medium, 7 Low, 6 architecture/quality** findings, and **5 contract findings** (one of
them Medium).

---

## High

### H1. A PDF with a looping or very deep bookmark tree crashes the whole server
`lectrice-infra/src/pdf/pdfium_text_source.rs:96-113`: `collect_siblings` calls itself once for each
nesting level (line 110). `MAX_OUTLINE_ITEMS` (20 000) limits how many items are collected, but it does
nothing about how deep the recursion goes. A bookmark whose `/First` points back to itself gives a chain
20 000 calls deep. Imports run on a tokio blocking thread, which has a 2 MiB stack. A stack overflow can't
be caught, so the process aborts. It doesn't just turn into a 500.

**Reproduced:** I wrote a hand-made 1-page PDF where outline item 6 has `/First 6 0 R`. I uploaded it with
`curl -F file=@loop.pdf localhost:7991/api/documents` to the release server (scratch data dir).
curl got an empty reply. The server log ended with
`thread 'tokio-rt-worker' has overflowed its stack / fatal runtime error: stack overflow, aborting`, and
the process was gone. The `clean` dev command also dies with a 2 MiB stack (`ulimit -s 2048`), and in
debug builds it dies even with 8 MiB.

**What it causes:** one broken or hostile PDF (cyclic outlines are a known way PDFs break) stops
playback, exports and every other request, and anything still in memory is lost.

**Fix:** walk the outline without recursion (an explicit stack of `(bookmark, level)`), and add a depth
cap such as `MAX_OUTLINE_DEPTH = 16` (levels are cut to 4 later anyway). Keep the item cap. Add a test
with a self-referencing `/First` (a hand-written PDF like the one above is ~30 lines).

---

## Medium

### M1. Audio requests the browser has given up on still run first, so a jump waits behind them
The chain:
- `lectrice-api/src/routes/extract.rs:47-48`: `run_blocking` uses `spawn_blocking`. When the client
  aborts, axum drops the handler future, but the blocking task keeps running.
- `lectrice-app/src/speech/speak_sentence.rs:141-143`: that task keeps blocking in `ticket.wait()`, and its
  job stays in the queue at `SpeechPriority::Now`.
- Nothing cancels the ticket when the request goes away.
- In the frontend, `hooks/usePlayback.ts:139` calls `buffer.keepOnly(wanted)`, which aborts every request
  outside the new window (`sentenceAudioBuffer.ts:51`), and each sentence change requests the current
  sentence plus `PREFETCH_AHEAD = 2`, all at `Now`.
- The worker runs jobs of the same priority first in, first out (`worker.rs:226-231`).

**Scenario:** the listener clicks sentence 900, then quickly 1500, then 2100, in parts that aren't cached
yet. Each click queues 3 `Now` jobs. The request for 2100 runs after the 6 abandoned jobs for 900–902 and
1500–1502, about 6–8 s at ≈ 1 s per sentence, plus the running read-ahead job (B5 already measured 1.8 s
for a single jump). The abandoned clips do get cached, but they belong to positions the listener has
left, and they eat into the cache window.

**Fix (pick one):**
1. Give `SpeakSentenceArgs` a cancellation flag (`Arc<AtomicBool>`). The controller sets it from a drop
   guard when the future is dropped. `receive` waits with `recv_timeout` in a loop and calls
   `ticket.cancel_handle().cancel()` once the flag is set. The worker already drops queued jobs that have
   no waiters.
2. Or keep a "current Now job per (document, voice)": a new `Now` request for that document and voice
   demotes the earlier `Now` jobs that haven't started to `ReadAhead`.

---

## Low

### L1. A zero-sized monospace font makes the cleaner panic (500), and a tiny one could exhaust memory
`lectrice-domain/src/cleaning/code.rs:40-55, 19-20, 32`: if no code piece has 3+ chars and a positive width,
`char_width` falls back to `line.size × 0.6`. That is `0.0` when the font size is 0. Then
`(gap / char_width).round() as usize` is `usize::MAX` and `" ".repeat(...)` panics with "capacity
overflow". `run_blocking` turns the panic into a 500 `Internal`, so the server survives. But a tiny
*positive* width (from a size near 0) asks for a huge but "valid" allocation instead, and Rust aborts the
process when an allocation fails. I didn't reproduce that variant.

**Found by the fuzz harness:** with zero-size runs, 15 683 of 20 000 random documents panicked inside
`code::line_text`. With realistic sizes there were 0 panics and no gaps in sentence ids.

**Fix:** set a minimum `char_width` (e.g. `MIN_CHAR_WIDTH = 1.0` pt) and cap the spaces one gap can add
(e.g. `MAX_GAP_SPACES = 80`). Also cap `indent` the same way.

### L2. `FsAudioCache::get` can drop a clip that was just written from the index
`lectrice-infra/src/storage/audio_cache.rs:166-176`: when the read fails with `NotFound` (the normal cache
miss), `get` then takes the lock and removes the index entry. If a read-ahead callback `put`s the same
key between the failed read and that lock, `get` deletes the brand-new entry. The file stays on disk but
isn't in `total_bytes`, so neither the window nor the cap ever evicts it until the next restart rescans.

**Scenario:** a request for sentence K arrives while the worker is finishing K's read-ahead job. The
window is small but real, and each time it happens one clip is lost from the accounting.

**Fix:** in the `NotFound` branch, re-check under the lock and remove only if the file still doesn't
exist. Or don't touch the index in `get` at all (`delete` already removes evicted clips).

### L3. The config allows keep window = read-ahead, which makes clips get deleted and made again
`lectrice-api/src/config/mod.rs:122-126` accepts `keep_minutes_per_document == read_ahead_minutes`.
The plan uses the 18 chars/s estimate, and real audio came out 6 % longer in B5's check. So the real
audio ahead can exceed the window. `retain_around` then deletes the farthest-ahead clips, and on the next
request the plan finds them missing and queues them again. That is wasted CPU on every sentence.

**Fix:** require `keep > read_ahead` with some margin (e.g. `keep ≥ read_ahead × 1.5` or + 5 min). Or
don't let the window evict clips inside the current read-ahead range.

### L4. `rate` is only checked in the route; a NaN rate inside the app would wipe a book's cache
`read_ahead.rs:124` computes `keep_seconds = keep × 60 × rate`, and `audio_cache.rs:200` does
`keep_seconds.max(0.0)`. `NaN.max(0.0)` is `0.0`, so a NaN rate would delete every clip of the scope
except the current one. Today the route rejects NaN, ±inf and anything outside 0.75–2, so HTTP can't
trigger this. But the rules ask for validation in both the route and the service, and a later
`lectrice-ffi` would call the app directly.

**Fix:** have `SpeakSentence::run` reject a rate that isn't finite or is ≤ 0 (`RequestInvalid`). Ideally,
make the rate a domain type that can only be built from a valid value.

### L5. Re-cleaning can overwrite a reading position saved while it ran
`lectrice-app/src/documents/pipeline.rs:45-51` (called from `get_document.rs:68` and `import_pdf.rs:76-78`):
the meta is read *before* the PDF is re-processed (which takes seconds), then written whole by
`FsDocumentRepository::save` (`document_repository.rs:151`). A `PUT …/position` that lands in between is
lost. It only happens after a `cleanerVersion` bump with two tabs open, so it is rare.

**Fix:** have `save` merge `last_sentence_id` from the current `meta.json` under `meta_lock`, or
re-read the summary just before saving.

### L6. Clip lengths and the silence clip hard-code Kokoro's 24 kHz WAV format
- `audio_cache.rs:17-18` (`WAV_HEADER_BYTES = 44`, `WAV_BYTES_PER_SECOND = 48 000`) works out each clip's
  length from its file size.
- `speak_sentence.rs:16` (`SILENCE_SAMPLE_RATE = 24_000`) hard-codes the engine's sample rate.

Both assume Kokoro's 24 kHz output and the WAV encoder. A different engine (spec: "swappable plug-in")
at 22.05 or 44.1 kHz would size the cache window wrong by up to 2×, and would play the silence clip at a
different rate from the rest of the book.

**Fix:** keep `audio_seconds` in the cache index, passed in by `put` or read from the WAV header during
the scan. For the silence clip, use the engine's sample rate: add `SpeechEngine::sample_rate()`, or
return an empty `Audio` from the engine (B1 already returns empty audio for blank text).

### L7. Callbacks on the worker thread run without a panic guard
`worker.rs:294-301`: `deliver` calls each `on_done` with no `catch_unwind`. Engine panics are caught
(`worker.rs:273-278`), but a panic in a callback (read-ahead encode and `put`, and B6's export callbacks)
kills the `speech-worker` thread. All the reply senders sit in `Shared`, which stays alive, so every
later `ticket.wait()` blocks forever. Audio requests then hang until the blocking pool runs out of
threads. Today's callbacks have no obvious panic path, so I rate this Low, but the failure would be total.

**Fix:** wrap each `(waiter.on_done)(outcome)` in `catch_unwind` and log a `warn`. Also fix the comment
on line 293 ("every waiter but the first"): the code gives the original error to the *last* waiter, and
B4.md says the *first*.

---

## Architecture and code quality

- **A1. app depends on `sha2`** (`lectrice-app/Cargo.toml:12`, used by `import_pdf.rs:122`). Spec §2 says
  the app depends on the domain only, and which hash to use is an adapter decision. Either add a small
  `ContentHasher` port with an infra adapter, or record it as a deliberate deviation. (`tracing` in the
  app is fine, as a cross-cutting concern.)
- **A2. Duplication:**
  - `sentence_count` exists twice (`app/documents/pipeline.rs:58`, `infra/storage/document_repository.rs:63`)
    with the same "cues included" rule. It belongs on `Document` in the domain.
  - `SECONDS_PER_MINUTE` is defined twice (`read_ahead.rs:15`, `read_ahead_plan.rs:7`).
  - `storage_failed` has 4 copies in infra (`document_repository.rs`, `audio_cache.rs`, and B6's
    `export_records.rs` and `ffmpeg.rs`). Move it into `storage/mod.rs`.
- **A3. Magic number:** `document_repository.rs:139` `options.limit.unwrap_or(20)`. The default page size
  needs a named constant. The fake repeats the literal (`app/documents/fakes.rs:179`).
- **A4. Unused dependency feature:** `tower-http` `"limit"` (`backend/Cargo.toml:38`). Nothing uses
  `RequestBodyLimit`, because the upload limit is enforced in the controller.
- **A5. Dead public API:** `ReadAheadPlan::ids()` (`read_ahead_plan.rs:29`) is `pub` and re-exported, but
  only tests use it. Make it `#[cfg(test)]` or a test helper.
- **A6. Validation only in the route** for `ListRecentDocuments` (limit 1–100, `routes/documents.rs:34-39`)
  and for `rate` (see L4). The rules' checklist asks for "inputs validated (route + service)".

Also worth a look:
- `CachedLibrary::find` (`cached_library.rs:69`) deep-clones the whole `Document` (a few hundred KB to a few
  MB) on every audio request. Returning `Arc<Document>` would make the cache really cheap.
- Each audio request writes about 4–6 `info` lines ("speaking sentence", "document loaded", "audio cache
  looked up", "read-ahead planned", and so on). The user asked to log only the important parts, so
  consider `debug` for the lookup lines.

The rest of the checklist holds:
- Dependencies point inward: domain has no infra or api deps; B0 documented its serde/chrono/regex
  deviation.
- Adapters are injected as `Arc<dyn Trait>`, and composition is the only place that wires them.
- Controllers are thin, with DTO validation and presenters per feature file.
- Errors are raised by name and turned into HTTP only in `lectrice-api/src/error.rs`. Codes are unique
  and tested.
- No file is a god module.
- No scratch tests, `dbg!` or `println!` are left (B2's `tmp_print.rs` is gone).
- **Logging:** I found no book text, PDF bytes or audio in any log or error detail. Details hold only
  ids, counts, paths and sizes. Malformed-JSON rejections echo the client's bad value, but that is client
  input, not book content.
- **Other correctness checks that passed:**
  - Read-ahead budget and cache window both scale with the rate (`× rate`).
  - Eviction order: farthest behind first, then farthest ahead. The current clip and clips in use are
    never deleted. The cap goes oldest first and spares the listener's own scope.
  - Lock order: plans → worker queue / cache; callbacks take cache → plans, never while holding the
    queue lock. No lock cycle.
  - No reference cycle between the worker and its callbacks.
  - Atomic writes for every file.
  - Ids that become paths are checked (`DocumentId` must be 64 hex chars; engine and voice ids go through
    `safe_component`; sentence ids are `u32`).
  - Unknown voice ids are rejected before sherpa-onnx is called.
  - `pdfium-render` has `thread_safe` on.

---

## Contract: frontend (`frontend/src/api`, `src/types`) ↔ backend

Matching:
- Paths, the `voice`/`rate`/`readSkipped` query names and their formats (`String(rate)`,
  `"true"`/`"false"`).
- Status codes (201/200 on import, 204 on position).
- `ListResponse`, `ImportResult`, `VoiceList`, `Health`, `DocumentSummary` and `OpenedDocument.lastSentenceId`
  (always present, `null` when unread).
- Block shape: `spoken` is left out for spoken blocks and `false` for skipped ones. `text` and `cue` are
  left out (not `null`) when absent. Heading `level` is always set and at most 4.
- Timestamps are whole-second ISO strings.
- Upload limits on both sides are 200 MiB.

Mismatches:

1. **Medium: the frontend doesn't know about silent sentences.** The backend sends `speechText: ""` for
   sentences that have nothing to say (bibliography and copyright pages, URL-only lines), and the
   audio route answers them with 0.25 s of silence. The frontend `Sentence` type (`types/document.ts:7`)
   has no `speechText`, and `buildPlaybackOrder` (`utils/playbackOrder.ts:23-40`) puts these sentences in
   the play order. The backend's read-ahead already leaves them out (`read_ahead_plan.rs:48-51`), so the
   two orders also differ.

   Measured with the `clean` dev command on *Clean Architecture*: 175 of 5 515 sentences are silent, and
   the longest run is **38 in a row**. That is 38 HTTP round trips with silence and a highlight crawling
   over unread text.

   **Fix (frontend, W step):** add `speechText?: string` to `Sentence`, and skip sentences with
   `speechText === ""` in `playableSentences`. While there, base `charsFrom`/"time left" on
   `speechText ?? text`.
2. **`RequestInvalid` (1000, 400) is missing from the frontend error names** (`types/errors.ts:3-7`,
   `api/errors.ts:3`). `client.ts:14-17` turns it into `Internal` with "Something went wrong." Add it to
   both lists, and to spec §8 (B4 deviation 1).
3. **Unknown paths and wrong methods** get axum's default 404/405 with an empty body, not the
   `{ error: { name, code, message } }` shape that spec §7 promises "always". The frontend maps them to
   `Internal`. Add a `Router::fallback` (and a method-not-allowed fallback) that returns a registry
   error.
4. **Export routes aren't there yet (B6):** `POST`/`GET /api/documents/{id}/exports`,
   `GET`/`DELETE /api/exports/{exportId}` and `GET /api/exports/{exportId}/file` are called by
   `api/exports.ts` but not in `routes/mod.rs` yet. The frontend expects `{ export: AudioExport }`,
   `{ items: AudioExport[] }` (newest first), `202` on start and `204` on cancel.
5. Small, no change needed: the backend requires `voice` on the audio route (400 `RequestInvalid` without
   it), and the frontend always sends it.
