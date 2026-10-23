# Lectrice: Rust stack research (2026-10-06)

Scope: pick the crates and toolchain for the Rust core (domain → infrastructure → application → api) of Lectrice, a local Speechify-like reader.
Method: versions come from the crates.io API on 2026-10-06. Crate source was downloaded and read: `sherpa-onnx`, `sherpa-onnx-sys`, `sherpa-rs`, `sherpa-rs-sys`, `kokoro-tts`, `pdfium-render`, `pdf_oxide`, `pdf-extract`, `mupdf`, `sentencex`, `srx`, `punkt`, `axum`, `tower-http`, `uniffi` and `tracing-subscriber`. sherpa-onnx C++ sources and examples were read on GitHub `master`. The Kokoro voice list was read straight from the metadata inside the local `model.onnx`.
Nothing was installed and no cargo command was run. Anything marked **UNVERIFIED** was not checked against source.

---

## 1. TTS binding: Kokoro through sherpa-onnx

### Options

| Crate | Version (date) | Status | Notes |
|---|---|---|---|
| **`sherpa-onnx`** (official, k2-fsa) | **1.13.8** (2026-09-11) | Active, Apache-2.0 | Safe wrapper over `sherpa-onnx-sys` 1.13.8. Has Kokoro, VITS, Matcha, Kitten, ZipVoice, Pocket and Supertonic TTS, plus ASR and VAD. |
| `sherpa-rs` (thewh1teagle) | 0.6.8 (2025-10-05) | **Archived on 2026-06-06.** The README says: *"The upstream project sherpa-onnx now provides an official Rust API, so this repository is no longer maintained. Please use the official Rust bindings instead."* | Pinned to sherpa-onnx v1.12.9. Always runs `bindgen`, which needs libclang. `create(&mut self, …)`. |
| `kokoro-tts` (mzdk100) | 0.3.3 (2026-06-01) | Active, Apache-2.0 | Pure Rust on `ort` 2.0.0-rc.12 with `cmudict-fast`, jieba and pinyin for G2P. It uses its **own model/voice format** (its own GitHub releases), so it can't use our sherpa bundle. Its API is async/tokio. |
| `kokoros` | not on crates.io | n/a | Only exists as a GitHub binary project. |
| `ort` directly | 2.0.0-rc.13 (2026-07-28) | Still an RC | You would have to write G2P yourself (espeak-ng via FFI, or misaki) plus tokenization and voice-style lookup. Too much work for little gain. |

### Official `sherpa-onnx` crate: verified facts

**Kokoro config** (`sherpa-onnx-1.13.8/src/tts.rs`, `OfflineTtsKokoroModelConfig`; every path field is `Option<String>`):

| Field | Value for our bundle | Notes |
|---|---|---|
| `model` | `…/kokoro-multi-lang-v1_0/model.onnx` | required |
| `voices` | `…/voices.bin` | required |
| `tokens` | `…/tokens.txt` | required |
| `data_dir` | `…/espeak-ng-data` | required. The C++ code checks for `phontab`, `phonindex`, `phondata` and `intonations`. |
| `lexicon` | `…/lexicon-us-en.txt` (or `lexicon-gb-en.txt`). Several files go in one comma-separated string, e.g. `"…/lexicon-us-en.txt,…/lexicon-zh.txt"`. | For a v1.0 (multi-lang) model you **must** set `lexicon` or `lang`. If both are empty, the C++ code calls `SHERPA_ONNX_EXIT(-1)`, which **kills the process**. |
| `lang` | e.g. `"en"`. Can be left empty if `lexicon` is set. | The C++ help text lists the values en, es, fr, hi, it, pt-br. It can be overridden per call with `GenerationConfig.extra["lang"]`. |
| `dict_dir` | leave `None` | The C++ code logs: *"From sherpa-onnx v1.12.15, you don't need to provide dict_dir"*. The official `kokoro_tts_zh_en.rs` example still passes it, which is harmless. |
| `length_scale` | `1.0` | Default speed. A larger value means slower speech. |

Outer config: `OfflineTtsModelConfig { kokoro, num_threads: i32, debug: bool, provider: Option<String> /* "cpu" | "cuda" */, .. }` and `OfflineTtsConfig { model, rule_fsts, rule_fars, max_num_sentences, silence_scale }`.
In the Rust struct, `max_num_sentences` defaults to 0 because of `derive(Default)`. The C API turns 0 into 1 (`SHERPA_ONNX_OR(config->max_num_sentences, 1)` in `c-api.cc`), and `silence_scale` becomes 0.2.
The `*.fst` files (`date-zh.fst`, `number-zh.fst`, `phone-zh.fst`) are Chinese text-normalization rules for `rule_fsts`. English doesn't need them.

**Generation:** `OfflineTts::generate_with_config(&self, text, &GenerationConfig, Option<callback>) -> Option<GeneratedAudio>`.
- `GenerationConfig { sid: i32, speed: f32 (default 1.0), silence_scale: 0.2, extra: Option<HashMap<String, Value>>, .. }`.
- `GeneratedAudio::samples() -> &[f32]` and `sample_rate() -> i32`.
- The optional callback `FnMut(&[f32], f32 progress) -> bool` gets chunks as they are produced. Returning `false` stops generation.
- On the engine, `OfflineTts::sample_rate()` and `num_speakers()` are available. There is **no speaker-name API**, so the name ↔ id map must live in our adapter.

**Sample rate:** **24000 Hz**, read from the model metadata (`sample_rate=24000`).

**Speed:** `GenerationConfig.speed` is a multiplier: >1 is faster, <1 is slower. If `speed == 1` and `length_scale != 1`, the engine uses `length_scale` instead (`offline-tts-kokoro-model.cc`).

**Gotchas:**
- `generate_with_config` runs `CString::new(text).unwrap()`, so text containing a NUL byte **panics**. Strip `\0` first.
- A `sid` outside 0..53 is **silently replaced with 0**, with only a log line. Validate it in the adapter.
- A bad model config can **exit the process** (see `lexicon`/`lang` above). Validate that the paths exist before calling `create`.
- Inside one generate call, tokens per chunk are capped at 510 (`style_dim 510,1,256`). The engine splits the text itself, but feeding one sentence at a time is safer anyway.

**Thread safety:** the crate declares `unsafe impl Send for OfflineTts {}` and `unsafe impl Sync for OfflineTts {}`, with the comment *"The sherpa-onnx C library is thread-safe for single-object usage."* `generate_with_config` takes `&self`.
The espeak-ng phonemizer is protected by a global `static std::mutex espeak_mutex` (`piper-phonemize-lexicon.cc`), and ONNX Runtime `Session::Run` is thread-safe.
So concurrent calls are allowed. On CPU they share cores and each call already uses `num_threads`, so concurrency gives little speed-up. **Recommendation:** run one dedicated TTS worker thread (or a semaphore of 1–2) over an `Arc<OfflineTts>`.

**Native libraries** (`sherpa-onnx-sys-1.13.8/build.rs`):
- Features: `static` (the default) or `shared`. You can't enable both.
- The build script **downloads prebuilt archives** from `https://github.com/k2-fsa/sherpa-onnx/releases/download/v1.13.8/…`:
  - Linux x64 static: `sherpa-onnx-v1.13.8-linux-x64-static-lib.tar.bz2` (21 MB)
  - Linux x64 shared: `…-linux-x64-shared-lib.tar.bz2` (9 MB)
- **No cmake, no clang and no bindgen are needed**, because the bindings are hand-written.
- Offline or custom builds: `SHERPA_ONNX_ARCHIVE_DIR` (a folder holding a pre-downloaded archive) or `SHERPA_ONNX_LIB_DIR` (a folder of already-built libs).
- Static mode links `sherpa-onnx-c-api`, `-core`, `kaldi-*`, `piper_phonemize`, `espeak-ng`, `onnxruntime` and others statically. It also links `stdc++`, `m`, `pthread` and `dl` dynamically, so **`gcc-c++`/`libstdc++-devel` must be installed** (neither is installed now).
- Shared mode adds `-Wl,-rpath` for the lib dir and `$ORIGIN`.

**CUDA:** the official crate has **no `cuda` feature**. The v1.13.8 release does ship `sherpa-onnx-v1.13.8-cuda-12.x-cudnn-9.x-onnxruntime1.28.2-linux-x64-gpu.tar.bz2` (374 MB) and a `cuda-13.x` variant. The likely path is the `shared` feature, `SHERPA_ONNX_LIB_DIR` pointing at the unpacked GPU `lib/`, and `provider: Some("cuda")`.
**UNVERIFIED:** that the GPU archive has the exact `lib/` layout the build script expects. It also needs system CUDA and cuDNN at runtime. (`sherpa-rs` had a `cuda` feature, but it forced a cmake build from source.)

**Mobile:** `build.rs` maps `android` (aarch64, arm, x86, x86_64) to `sherpa-onnx-v1.13.8-android.tar.bz2` (jniLibs per ABI) and `ios` to `…-ios-shared-onnxruntime-static.xcframework.zip`. Both force **shared** linking. On Android it tries to copy the `.so` files into a Tauri-style `gen/android/app/src/main/jniLibs` folder and otherwise leaves them in the cache, so a uniffi/Gradle setup must package `libsherpa-onnx-c-api.so` and `libonnxruntime.so` itself. **UNVERIFIED** end to end.

### Voice name → speaker id (authoritative)

This list comes from the `speaker2id` / `speaker_names` metadata embedded in the local `models/kokoro-multi-lang-v1_0/model.onnx`, written by k2-fsa's `add_meta_data.py`. The bundle README confirms 54 voices, and that `em_santa` was appended as id 53 while ids 0–52 stayed the same.

| id | name | id | name | id | name | id | name | id | name | id | name |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 0 | af_alloy | 9 | af_sarah | 18 | am_puck | 27 | bm_lewis | 36 | im_nicola | 45 | zf_xiaobei |
| 1 | af_aoede | 10 | af_sky | 19 | am_santa | 28 | ef_dora | 37 | jf_alpha | 46 | zf_xiaoni |
| 2 | af_bella | 11 | am_adam | 20 | bf_alice | 29 | em_alex | 38 | jf_gongitsune | 47 | zf_xiaoxiao |
| 3 | af_heart | 12 | am_echo | 21 | bf_emma | 30 | ff_siwis | 39 | jf_nezumi | 48 | zf_xiaoyi |
| 4 | af_jessica | 13 | am_eric | 22 | bf_isabella | 31 | hf_alpha | 40 | jf_tebukuro | 49 | zm_yunjian |
| 5 | af_kore | 14 | am_fenrir | 23 | bf_lily | 32 | hf_beta | 41 | jm_kumo | 50 | zm_yunxi |
| 6 | af_nicole | 15 | am_liam | 24 | bm_daniel | 33 | hm_omega | 42 | pf_dora | 51 | zm_yunxia |
| 7 | af_nova | 16 | am_michael | 25 | bm_fable | 34 | hm_psi | 43 | pm_alex | 52 | zm_yunyang |
| 8 | af_river | 17 | am_onyx | 26 | bm_george | 35 | if_sara | 44 | pm_santa | 53 | em_santa |

How to read the names: the first letter is the language (a = US English, b = UK English, e = Spanish, f = French, h = Hindi, i = Italian, j = Japanese, p = Brazilian Portuguese, z = Mandarin). The second letter is f (female) or m (male).
For an English reader, ids 0–27 (a*/b*) are the relevant ones. Use `lexicon-us-en.txt` for a* voices and `lexicon-gb-en.txt` for b* voices; the lexicon is fixed per engine instance (`lang` can change per call).

Other metadata found: `model_type=kokoro`, `voice=en-us` (the default lang), `n_speakers=54` and `style_dim=510,1,256`.

**Recommendation:** use the official **`sherpa-onnx = "1.13.8"`** crate with default features (`static`). It is maintained upstream, matches our bundle, needs no cmake or clang, has prebuilt libs for Linux, Android and iOS, and is `Send + Sync` with `&self` generation. `sherpa-rs` is archived. Keep the name ↔ id table in the Kokoro adapter, either hard-coded or parsed from the ONNX metadata.

---

## 2. PDF text extraction with layout

| Crate | Version | License | Native deps | Per-run data we need | Notes |
|---|---|---|---|---|---|
| **`pdfium-render`** | 0.9.4 (2026-09-06) | MIT OR Apache-2.0. PDFium itself is BSD-3-Clause (Google). | Needs `libpdfium.so` at runtime (`Pdfium::bind_to_library` / `bind_to_system_library`) or a `static` feature. | `PdfPageText::segments()` gives text + `bounds()`. `chars()` gives `unicode_char`, `scaled_font_size`, `font_name`, `font_weight`, `tight_bounds`/`loose_bounds`, `origin`, **`is_hyphen()`** and **`is_generated()`**. Page index comes from page iteration. | Mature (~3M downloads), uses Chrome's PDF engine. Default features: `pdfium_latest` (=`pdfium_7881`), `image_latest` and **`thread_safe`**, which wraps all PDFium calls in a mutex because PDFium is not thread-safe. |
| `pdf_oxide` | 0.3.78 (2026-09-08) | MIT OR Apache-2.0 (the name is trademarked) | Pure Rust by default. `ort`/`pdfium-render` are optional features. | `extract_spans(page) -> Vec<TextSpan>` with `text`, `bbox`, `font_name`, `font_size`, `font_weight`, `is_italic` and `artifact_type` (tagged header/footer artifacts). It also has `extract_spans_with_reading_order` (XY-cut), `extract_chars`, `extract_words` and a hyphenation-reconstruction config. | Very feature-rich, but **young** (repo created 2025-11-05, 78 patch releases in 0.3.x, one main maintainer) and has a very large dependency tree. |
| `pdf-extract` | 0.12.1 (2026-09-16) | MIT | Pure Rust (on `lopdf`) | An `OutputDev` trait: `output_character(trm, width, spacing, font_size, char)`, `begin_page(page_num, media_box, …)`. You build runs yourself. | Low-level. Known to be weaker on unusual fonts and encodings (UNVERIFIED for this project). |
| `lopdf` | 0.45.0 | MIT | Pure Rust | No layout. It is an object-level PDF parser. | Not usable on its own. |
| `mupdf` | 0.8.0 (2026-06-22) | **AGPL-3.0** | Builds MuPDF from C source (`mupdf-sys`) | Good structured text | AGPL: shipping the app, including on mobile, would require releasing it under AGPL, or buying Artifex's commercial license. **Avoid.** |

**Getting libpdfium:**
- Prebuilt binaries are at https://github.com/bblanchon/pdfium-binaries/releases. The latest is `chromium/8086` (2026-10-05). Pin **`chromium/7881`**, which matches `pdfium_latest` in pdfium-render 0.9.4 (the tag exists).
- Assets: `pdfium-linux-x64.tgz`, `pdfium-android-arm64.tgz`, and `pdfium-ios-device-arm64.tgz` / `-simulator-*`. The build scripts are MIT.
- Fedora has no `pdfium` package. Put `libpdfium.so` in e.g. `vendor/pdfium/lib/` and load it by path.

**What we build ourselves either way:** header/footer removal (the same text at a similar y-band across many pages), page-number regex, dropping small fonts below the body's median font size, de-hyphenation (`is_hyphen()` in pdfium, or a line-end `-` followed by a lowercase letter), and column ordering (cluster by x, then sort by y). These are pure domain logic over a `TextRun { page, text, font_size, bbox }` list, so they can be unit-tested without any PDF library.

**Recommendation:** use **`pdfium-render` 0.9.4 + pdfium-binaries `chromium/7881`** behind a `DocumentParser` trait. It is the most robust extractor, permissively licensed, and has binaries for Linux, Android and iOS. Keep **`pdf_oxide`** as the fallback or comparison candidate, since it is pure Rust and needs no `.so`. A one-day spike on 3–5 real PDFs is worth it before committing. Don't use `mupdf` because of AGPL.

---

## 3. Sentence splitting (English)

| Option | Version | License | Abbreviations (e.g., Dr., Fig. 3) | Notes |
|---|---|---|---|---|
| **`sentencex`** (Wikimedia) | 0.1.32 (2026-09-28) | MIT | Has an `abbrev/en.txt` list that includes `e.g`, `i.e`, `dr`, `fig`, `etc`, `vs`, `al`, `pp`, `no`. It doesn't split before a lowercase letter or digit, so "Fig. 3" and "e.g. the" stay together. It also handles digit rules and ellipses. | API: `segment("en", text) -> Vec<&str>` and `get_sentence_boundaries("en", text)`, which returns byte and char offsets plus `is_paragraph_break` (useful for highlight sync). Requires **Rust ≥ 1.91**, edition 2024. Pulls in `clap` as a normal dependency (minor bloat). Supports many languages. |
| `unicode-segmentation` (`split_sentence_bounds`) | 1.13.3 | MIT/Apache | UAX #29 has no abbreviation list. It doesn't break when a lowercase word follows ("e.g. the"), but **does break "Dr. Smith" and "Fig. 3"**, because an uppercase letter or digit after a period counts as a boundary. | Zero-config, but wrong for our cases. |
| `srx` | 0.1.4 (2023-07) | MIT/Apache | Rule-based. Needs an SRX rules file, e.g. LanguageTool's `segment.srx` (LGPL data, and some of its rules are skipped as unsupported). | Not maintained since 2023. |
| `punkt` | 1.0.5 (2019) | MIT/Apache | Trained model | Uses `#![feature(proc_macro_hygiene)]`, so it **needs nightly**, and it is unmaintained. **Avoid.** |

**Recommendation:** use **`sentencex` 0.1.32** behind a `SentenceSplitter` trait. Add a small custom pre/post pass with our own extra abbreviations and protection for things like "et al.", "Eq. 2" and decimals. Write unit tests with the exact cases: "e.g.", "Dr.", "Fig. 3", "U.S.", "3.14", and quotes after a period.

---

## 4. HTTP layer

| Item | Choice | Verified details |
|---|---|---|
| Framework | **`axum` 0.8.9** (2026-04-14) | rust-version 1.80. Default features: http1, json, query, tokio and tracing. **Enable the `multipart` feature** (it uses `multer`). |
| Middleware | **`tower-http` 0.7.1** (2026-08-31) | Same `http` 1.x and `tower` 0.5 as axum 0.8, so they are compatible. Use features `cors`, `trace`, `limit`, and `fs` if needed. 0.7.0 added a `csrf` layer. |
| Runtime | `tokio` 1.53.2 | |
| WAV | **`hound` 3.5.1** | Stable (last release 2023, 20M downloads). Write a 16-bit PCM mono 24 kHz WAV into a `Cursor<Vec<u8>>`. That is about 48 KB per second of audio. A browser `<audio>` element plays it natively. |

**How to serve per-sentence audio** (a design suggestion, not a measured result):
- `GET /documents/{id}/sentences/{n}/audio?voice=af_heart&speed=1.0` returns `200 audio/wav` with the full sentence.
- Cache the result to disk, keyed by a hash of (text, voice, speed, model version). Prefetch the next N sentences in the background.
- Per-sentence WAV is simpler than streaming, and with sentence-sized requests latency equals one sentence's synthesis time.
- If you want streaming later, the sherpa callback gives you chunks you could send via `axum::body::Body::from_stream`, but WAV headers need the length up front (or a 0xFFFFFFFF "unknown" length). That makes chunked streaming awkward, so start with whole-sentence WAV.

**CORS** for the Vite dev server: `CorsLayer::new().allow_origin("http://localhost:5173".parse::<HeaderValue>()?).allow_methods([GET, POST, DELETE]).allow_headers([CONTENT_TYPE])`. A cleaner alternative is Vite's `server.proxy` to the backend, which avoids CORS entirely during development.

**PDF upload:**
- Use the `axum::extract::Multipart` extractor.
- Its doc says: *"by default, `Multipart` limits the request body size to 2MB"*. PDFs are often larger, so add `DefaultBodyLimit::max(100 * 1024 * 1024)` (or similar) on the upload route.
- Stream fields to a temp file instead of holding them in memory.

**CPU-heavy TTS off the async runtime:**
- Preferred: a **dedicated worker thread** that owns an `Arc<OfflineTts>`. It receives jobs on a `tokio::sync::mpsc` channel (or `crossbeam`) and replies on a `oneshot`. This gives natural serialization, a queue you can prioritize (current sentence first, then prefetch) and cancel, and nothing blocks tokio's blocking pool.
- `tokio::task::spawn_blocking` also works for one-off calls such as PDF parsing. Note that pdfium-render's `thread_safe` mutex serializes PDF work anyway.

**Recommendation:** use axum 0.8.9 + tower-http 0.7.1 (`cors`, `trace`, `limit`) + hound 3.5.1. Use per-sentence WAV with a disk cache. Run a dedicated TTS worker thread with an mpsc job queue. Use `spawn_blocking` for PDF parsing.

---

## 5. Errors and logging

**Error registry** (one place, raised by name, mapped at the transport layer). Suggested shape, using `thiserror` 2.0.21 for the source chain:

```rust
// domain/src/error.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorName { DocumentNotFound, PdfUnreadable, VoiceUnknown, TtsFailed, UploadTooLarge, Internal }

pub struct ErrorSpec { pub code: u32, pub status: u16, pub operational: bool, pub description: &'static str }

impl ErrorName {
    pub const fn spec(self) -> ErrorSpec {
        match self {
            Self::DocumentNotFound => ErrorSpec { code: 1001, status: 404, operational: true,  description: "Document does not exist" },
            Self::VoiceUnknown     => ErrorSpec { code: 2001, status: 400, operational: true,  description: "Unknown voice id" },
            Self::TtsFailed        => ErrorSpec { code: 2002, status: 500, operational: false, description: "Speech synthesis failed" },
            // ...
            _ => ErrorSpec { code: 9999, status: 500, operational: false, description: "Internal error" },
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("{name:?}: {message}")]
pub struct AppError {
    pub name: ErrorName,
    pub message: String,
    #[source] pub source: Option<Box<dyn std::error::Error + Send + Sync>>,
}
// services: Err(AppError::new(ErrorName::VoiceUnknown, format!("voice={v}")))
```

- Keep the HTTP status as a plain `u16` so the domain crate doesn't depend on `http`.
- The api crate does `impl IntoResponse for ApiError(AppError)`. It looks up `spec()`, builds the `StatusCode` and returns JSON `{ "error": { "name", "code", "message" } }`.
- For non-operational errors it sends a generic message and logs the full source chain.

**Logging:** use `tracing` 0.1.44 + `tracing-subscriber` 0.3.23 with features `json` and `env-filter` (both exist in its Cargo.toml):
- Set up with `tracing_subscriber::fmt().json().flatten_event(true).with_env_filter(EnvFilter::from_default_env()).init()`. This writes one JSON object per line.
- At call boundaries, use `#[tracing::instrument(skip(self), fields(doc_id, voice))]` on use-case functions and adapter methods.
- Use `tower_http::trace::TraceLayer` for each HTTP request and response.
- Log errors once, at the transport boundary, with `error.name`, `error.code` and `operational`.
- `anyhow` isn't needed in library crates. It is fine in `main`/tests.

**Recommendation:** use a `thiserror` `AppError { name: ErrorName, … }` with a `const fn spec()` registry in the domain crate, mapped to the wire in api, plus `tracing` + `tracing-subscriber` (`json`, `env-filter`) + `TraceLayer`.

---

## 6. Later: MP3 export and mobile

| Crate | Version | License | Notes |
|---|---|---|---|
| `mp3lame-encoder` | 0.2.5 (2026-08-20) | **LGPL-3.0** (wraps `mp3lame-sys` 0.1.11, LGPL-3.0, which builds LAME from C source with `cc`) | The de facto choice. LGPL with static linking means you must allow relinking. Simplest compliance is dynamic linking. |
| `shine-rs` | 0.1.5 (2026-09-23) | LGPL-2.0 | Pure Rust port of shine (fixed-point, lower quality). Young. |
| `lame` | 0.1.3 (2017) | MIT (binding only; LAME is still LGPL) | Unmaintained. |

Alternative: Opus/Ogg (BSD-licensed libopus), e.g. the `opus` / `audiopus` crates (UNVERIFIED versions). Or call the user's own `ffmpeg` binary for exports. Browsers play WAV, so MP3 is only needed for "download as audiobook".

**uniffi:** **0.32.2** (2026-09-23), MPL-2.0 (license UNVERIFIED in this pass). It has a `tokio` feature for async. Use proc-macros (`#[uniffi::export]`) in a separate `lectrice-ffi` crate.
- Official `sherpa-onnx` supports `android` and `ios` targets with prebuilt libs (see §1), so it is mobile-capable.
- `sherpa-rs` is archived, so it is irrelevant.
- `pdfium-render` + pdfium-binaries have Android and iOS builds.
- **UNVERIFIED:** a full Android build (`cargo-ndk`, packaging both sherpa `.so` files and `libpdfium.so`) has not been tried.

---

## 7. Workspace layout and toolchain

### Suggested Cargo workspace

```
lectrice/
├─ Cargo.toml                # [workspace] members = ["crates/*"], resolver = "3", shared [workspace.dependencies]
├─ crates/
│  ├─ lectrice-domain/       # entities (Document, Sentence, TextRun, Voice), traits (SpeechEngine, DocumentParser,
│  │                         # SentenceSplitter, AudioEncoder, DocumentRepository, AudioCache), ErrorName/AppError.
│  │                         # Deps: thiserror only.  Pure logic: header/footer filter, dehyphenation, column ordering.
│  ├─ lectrice-infra/        # adapters: KokoroSherpaEngine (sherpa-onnx), PdfiumParser (pdfium-render),
│  │                         # SentencexSplitter, WavEncoder (hound), FsCache/FsRepository. Feature flags per adapter.
│  ├─ lectrice-app/          # use cases: ImportPdf, GetSentences, SynthesizeSentence, ListVoices; TTS worker queue.
│  │                         # Depends on domain only (adapters injected as Arc<dyn Trait>).
│  ├─ lectrice-api/          # axum binary: routes, DTOs, ApiError→IntoResponse, CORS, tracing init, wiring (composition root).
│  └─ lectrice-ffi/          # (later) uniffi bindings over lectrice-app
├─ vendor/pdfium/            # libpdfium.so (+ LICENSE) from pdfium-binaries chromium/7881
├─ models/kokoro-multi-lang-v1_0/
└─ web/                      # Vite + React + TS (separate npm project)
```

The adapters could also be split into one crate each (`lectrice-tts-sherpa`, `lectrice-pdf-pdfium`), so the mobile build can leave some out. Either way, keep the native dependencies out of domain and app.

### Fedora 44 install commands (checked with `dnf repoquery`, not executed)

```bash
# C/C++ toolchain: gcc is present, g++/libstdc++-devel are NOT; sherpa-onnx static links libstdc++
sudo dnf install gcc-c++

# Rust: Fedora ships rust/cargo 1.98.1, but sentencex needs >=1.91 and rustup gives stable 1.99 + targets for Android/iOS later
sudo dnf install rustup
rustup-init -y            # installs stable (1.99.0 on 2026-09-28)
source "$HOME/.cargo/env"

# Node for the Vite frontend (Fedora 44 has nodejs22 and nodejs24; npm comes from nodejs24-npm)
sudo dnf install nodejs24 nodejs24-npm

# PDFium runtime library
mkdir -p vendor/pdfium && curl -L https://github.com/bblanchon/pdfium-binaries/releases/download/chromium%2F7881/pdfium-linux-x64.tgz | tar xz -C vendor/pdfium
```

The following are **not needed** for the recommended stack: `cmake`, `clang-devel` (only `sherpa-rs`/bindgen needed them), and `openssl-devel` (UNVERIFIED: the sherpa sys build uses `ureq` 2 with its default rustls TLS).
The first `cargo build` needs network access to download the sherpa-onnx archive (21 MB) from GitHub, unless you set `SHERPA_ONNX_ARCHIVE_DIR`.

---

## Risks / unknowns

1. **sherpa-onnx can kill the process.** `SHERPA_ONNX_EXIT(-1)` runs on a bad Kokoro config (missing lexicon/lang) and on internal shape errors. It also panics on NUL in text. Mitigate by validating paths, lang and voice ids before calling, and by stripping `\0`.
2. **Bad voice ids fall back to voice 0** instead of failing. Validate them against the 54-name table.
3. **CUDA path is unverified.** There is no cargo feature. It relies on `shared` + `SHERPA_ONNX_LIB_DIR` + the 374 MB GPU archive + system CUDA 12/13 and cuDNN 9, and the NVIDIA driver isn't loaded right now.
4. **libpdfium is an extra runtime `.so`.** Version 7881 must match the `pdfium_7881` bindings. PDFium calls are serialized by a global mutex.
5. **PDF quality varies.** Header/footer, column and footnote heuristics are custom code and need a test corpus. pdf_oxide is a backup, but it is young and changes fast.
6. **Mobile builds are untested.** Packaging the sherpa `.so` files and pdfium for Android/iOS through uniffi hasn't been tried.
7. **Some licenses need attention.** MP3 via LAME is LGPL; Kokoro weights are Apache-2.0 (see the bundle `LICENSE`, not re-read here); espeak-ng-data is GPL-3.0, so **check redistribution terms before shipping a mobile app** (UNVERIFIED).
8. **`sentencex` needs Rust ≥ 1.91**, so use rustup stable, not an older distro rustc.
9. **Version churn.** `ort` is still an RC, and pdf_oxide puts out a release roughly weekly. Pin exact versions in the workspace.

## Sources

- crates.io API, 2026-10-06: https://crates.io/api/v1/crates/{sherpa-onnx, sherpa-onnx-sys, sherpa-rs, kokoro-tts, ort, pdfium-render, pdf_oxide, lopdf, pdf-extract, mupdf, sentencex, srx, punkt, unicode-segmentation, axum, tower-http, tokio, hound, thiserror, tracing, tracing-subscriber, uniffi, mp3lame-encoder, mp3lame-sys, shine-rs, lame}
- Official Rust bindings (source read from the 1.13.8 crate): https://github.com/k2-fsa/sherpa-onnx, https://docs.rs/sherpa-onnx
- Example: https://github.com/k2-fsa/sherpa-onnx/blob/master/rust-api-examples/examples/kokoro_tts_en.rs
- Example: https://github.com/k2-fsa/sherpa-onnx/blob/master/rust-api-examples/examples/kokoro_tts_zh_en.rs
- C API defaults: https://github.com/k2-fsa/sherpa-onnx/blob/master/sherpa-onnx/c-api/c-api.cc
- C++: `sherpa-onnx/csrc/offline-tts-kokoro-impl.h`, `offline-tts-kokoro-model.cc`, `offline-tts-kokoro-model-config.cc`, `piper-phonemize-lexicon.cc` (same repo, `master`)
- Release assets v1.13.8: https://github.com/k2-fsa/sherpa-onnx/releases/tag/v1.13.8
- sherpa-rs archive notice: https://github.com/thewh1teagle/sherpa-rs
- Local `models/kokoro-multi-lang-v1_0/model.onnx` metadata and `README.md`
- pdfium-render: https://github.com/ajrcarey/pdfium-render. Binaries: https://github.com/bblanchon/pdfium-binaries/releases (tags chromium/7881, chromium/8086). PDFium license: https://pdfium.googlesource.com/pdfium/+/refs/heads/main/LICENSE
- pdf_oxide: https://github.com/yfedoseev/pdf_oxide. pdf-extract: https://github.com/jrmuizel/pdf-extract. mupdf-rs: https://github.com/messense/mupdf-rs
- sentencex: https://github.com/wikimedia/sentencex. srx: https://github.com/bminixhofer/srx. punkt: https://github.com/ferristseng/rust-punkt
- axum: https://github.com/tokio-rs/axum (`src/extract/multipart.rs` 2MB note). tower-http changelog: https://github.com/tower-rs/tower-http/blob/main/tower-http/CHANGELOG.md
- uniffi: https://github.com/mozilla/uniffi-rs. mp3lame-encoder: https://github.com/DoumanAsh/mp3lame-encoder. shine-rs: https://github.com/wshon/shine-rs
- Rust stable channel: https://static.rust-lang.org/dist/channel-rust-stable.toml
- Fedora 44 packages: `dnf repoquery` (rustup 1.29.0, rust 1.98.1, nodejs24 24.18.0, nodejs24-npm, gcc-c++ not installed)
