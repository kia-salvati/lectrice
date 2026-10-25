# Lectrice for Android: design spec (2026-10-23)

Status: **draft for the user's review.** Nothing is built yet. Items marked **(proposed)** were not asked for; keep them or
drop them during review. Anything not checked is marked **UNVERIFIED**.

Inputs: `research/2026-10-23-android-options.md` (the main input; "the report" below), `docs/2026-10-21-design-spec.md`
("the web spec"), `docs/2026-10-23-chapters-brief.md`, `docs/progress/B0.md`, `rules/`.

Decisions this spec builds on (user, 2026-10-23):

- **Kotlin + Jetpack Compose, applying Telegram's principles. Never copy Telegram code** (it is GPL). We copy ideas only.
- The Rust core does the product logic, through a `lectrice-ffi` crate (uniffi).
- Test phones: **Galaxy S10+** (2019) and **Galaxy S26 Ultra** (current flagship). Other Android phones are available.
- **No server option at all** (user, later the same day). The earlier "phase 2 `RemoteEngine`" idea is dropped. Every fallback
  runs on the phone.
- **The Android app is a separate project and repo** (`lectrice-android`). It uses the core crates from the main repo (§3).
  **Sync confirmed by the user (2026-10-23):** one shared Rust core, used as Cargo git dependencies pinned to release tags,
  with a local path override for side-by-side work. Never fork the core.
- Chapters on the phone use the **bottom-sheet** form from the chapters work (the web also gets the whole-book rail; the phone
  does not).

---

## 1. Scope of Android v1

**Same features as web v1, plus chapters:**

1. Open a PDF (system file picker, or "Open with Lectrice" from another app). It is cleaned **once** and saved in the library on
   the phone, keyed by the SHA-256 of the bytes. The same Rust cleaner runs, so the result matches the desktop.
2. A library screen with recent documents, reading progress and "Continue".
3. A reader (design D): large text, page markers ("p. 37", never spoken), the current sentence highlighted, tap a sentence to
   jump there, follow-scroll plus a "locate" button.
4. Playback in the `af_heart` voice by default. Previous / play-pause / next, speed 0.75–2×, voice picker, and a
   "Read code & tables aloud" switch. Cues ("Code example.", "Table.") work as on the web.
5. **Background playback:** it keeps playing with the screen off, with lock-screen, notification, Bluetooth and headset controls.
6. Read-ahead (default 10 listening-minutes, scaled by speed) and a cache window of about an hour per book.
7. **Chapters:** the current chapter next to the page number, plus a chapters bottom sheet (chapters and sub-chapters, tap to
   jump).
8. **Export the whole book** as one **M4B** file with chapter marks, in the background, resumable.
9. First-run **model download**: the user picks full quality (fp32, about 370 MB) or smaller (int8, about 126 MB), or imports the model from a file, with a progress screen.

**Left out of Android v1:**

- MP3 export. Android has no MP3 encoder, and LAME is LGPL. M4B only (§8).
- Everything already out of web v1: GPU, voice cloning, the "show what was removed" toggle, deleting documents, OCR, languages
  other than English.
- Any server, sync with the desktop, or cloud. The app is fully local after the one-time model download.
- Tablets and foldables get the phone layout (they work, but are not tuned). No Android Auto, Wear or widgets.
- iOS. The core stays ready for it (uniffi Swift), but nothing is built for it.
- Only the `arm64-v8a` ABI at first. Older 32-bit phones and x86 emulators are not supported; the emulator is not needed,
  because we test on real phones.

**Platform targets:** `minSdk 29` (Android 10), `targetSdk` = the newest stable API at build time (36 or newer, UNVERIFIED).
The S10+ ends at Android 12 (API 31/32), so it is supported.

---

## 2. Architecture in one picture

```
            Kotlin (lectrice-android repo)                                Rust
┌──────────────────────────────────────────────────────────┐   ┌───────────────────────────────┐
│ feature/library   feature/reader   feature/settings  (ui)│   │ lectrice-ffi  (Android repo)   │
│        │ ViewModels = use cases, StateFlow               │   │  composition root + uniffi API │
│        ▼                                                 │   │  + adapters for Kotlin ports   │
│ core/model   (domain: models + interfaces, pure Kotlin)  │   └──────────────┬────────────────┘
│        ▲ implemented by                                  │                  │ git dependency, pinned tag
│ core/data  (the ONLY module that sees uniffi code)  ─────┼─ JNA ──►  lectrice-app / -infra / -domain
│ core/playback (Media3)   core/export (WorkManager,       │              (main lectrice repo)
│ MediaCodec)   core/designsystem (theme + components)     │
│ app (MainActivity, AppContainer = composition root)      │
└──────────────────────────────────────────────────────────┘
```

The rule is the same as in the backend and web rules: **dependencies point inward.** UI depends on domain interfaces, data
implements them, and only `app` knows the concrete classes.

---

## 3. Two repos, and how they stay in sync

### 3.1 Layout

```
~/Desktop/learning/
  lectrice/                       main repo (public): desktop app
    backend/crates/
      lectrice-domain/            shared core, no platform code
      lectrice-app/               shared core (use cases, speech worker, export runner)
      lectrice-infra/             shared adapters, desktop-only ones behind features (§5)
      lectrice-api/               desktop shell (axum server)
    frontend/
  lectrice-android/               NEW repo: the Android app
    Cargo.toml                    Rust workspace with one member: rust/lectrice-ffi
    Cargo.lock                    committed (it pins the core's commit)
    rust/lectrice-ffi/            uniffi API, composition root, Android-only Rust adapters
    .cargo/config.toml            gitignored; local path override (§3.3)
    native/                       fetch-libs.sh + SHA256SUMS for libsherpa-onnx-c-api.so, libonnxruntime.so, libpdfium.so
    settings.gradle.kts
    gradle/libs.versions.toml     every version pinned in one place
    app/  core/{model,data,playback,export,designsystem}/  feature/{library,reader,settings}/
    benchmark/  baselineprofile/  (M2+)
    spike/                        M1 only: the go/no-go app (docs/2026-10-23-android-m1-spike-plan.md)
    docs/                         Android progress reports
```

### 3.2 The dependency

`rust/lectrice-ffi/Cargo.toml`:

```toml
[dependencies]
lectrice-domain = { git = "https://github.com/kia-salvati/lectrice", tag = "v0.2.0" }
lectrice-app    = { git = "https://github.com/kia-salvati/lectrice", tag = "v0.2.0" }
lectrice-infra  = { git = "https://github.com/kia-salvati/lectrice", tag = "v0.2.0", default-features = false, features = ["pdfium", "kokoro"] }
uniffi = "=0.32.2"
```

- Cargo finds each crate by name inside the git repo, and reads the core's `[workspace.dependencies]` for the
  `workspace = true` lines. So the Android build gets the same exact (`=`) versions as the desktop.
- Both repos are on GitHub and public: `kia-salvati/lectrice` (tag `v0.1.0` pushed) and `kia-salvati/lectrice-android`
  (local clone at `../lectrice-android`, branch `main`). Tag `v0.1.0` doesn't have the changes in §3.5 yet, so **until
  `v0.2.0` exists the spike builds through the path override below.** (Versions were renumbered to beta 0.x on
  2026-10-24: `v1.0.0` → `v0.1.0`, `v1.1.0` → `v0.2.0`.)

### 3.3 Developing both side by side

A gitignored `lectrice-android/.cargo/config.toml` (a committed `config.toml.example` shows it):

```toml
[patch."https://github.com/kia-salvati/lectrice"]
lectrice-domain = { path = "../lectrice/backend/crates/lectrice-domain" }
lectrice-app    = { path = "../lectrice/backend/crates/lectrice-app" }
lectrice-infra  = { path = "../lectrice/backend/crates/lectrice-infra" }
```

With it, a core change is picked up at once, with no tag needed. Remove it (or build in CI) to check the pinned tag.

### 3.4 How the two repos stay in sync

1. **The core is a library with a public contract.** That contract is the public items of `lectrice-domain`, `lectrice-app` and
   the `lectrice-infra` adapters, plus the saved formats (`document.json`, `meta.json`, the audio cache layout, export records).
2. **Semver tags on the main repo** (`v0.2.0`, `v0.3.0`, …), with the version in `[workspace.package]` kept equal to
   the tag. While in beta (0.x), a breaking change to a trait, a public type or a saved format raises the minor version, and
   everything else raises the patch version.
3. **`CHANGELOG.md` in the main repo**, with a "Core" section per release. Breaking changes get a "Migration" note.
4. **Core changes go to the main repo first**, with their tests: domain/app/infra tests, and the desktop still builds. Then
   tag. Then the Android repo bumps `tag = "…"`, runs `cargo update -p lectrice-app -p lectrice-domain -p lectrice-infra`,
   regenerates the Kotlin bindings, builds, and runs its tests.
5. **Never fork core logic into the Android repo.** If Android needs a behaviour change (not just a platform adapter), it goes
   into the core. The Android repo holds only: the ffi API, the composition root, the adapters for Android ports (export
   encoder sink, logging) and Kotlin.
6. **`cleanerVersion` and `engine_id` belong to the core.** Both apps produce the same saved document for the same PDF and
   cleaner version.
7. Each Android release notes the core tag it was built with (`lectrice-android` `CHANGELOG.md`, "Core: v0.2.0").

### 3.5 Core changes needed for a clean split (all in the main repo, released as `v0.2.0`)

| # | Change | Why |
|---|---|---|
| C1 | **Feature flags on `lectrice-infra`:** `pdfium`, `kokoro`, `sherpa-static`, `ffmpeg-export`. `default = ["pdfium", "kokoro", "sherpa-static", "ffmpeg-export"]` (the desktop gets what it has today). The workspace `sherpa-onnx` dependency drops `features = ["static"]`; infra's `sherpa-static` feature turns it on. | Android builds infra without ffmpeg and with sherpa's shared libs. (axum, tower-http, tokio and tracing-subscriber are already only in `lectrice-api`, so nothing pulls them in; the ffi crate just doesn't depend on `lectrice-api`.) |
| C2 | **Move the wiring out of `lectrice-api`:** `Adapters`, `Settings`, `assemble()` and the `DocumentServices` / `SpeechServices` / `ExportServices` structs go to `lectrice-app::services` (it depends only on domain). `lectrice-api` keeps `build_state(config)`, which picks adapters from its TOML config. | Two composition roots (api, ffi) call one `assemble`, so they can't drift. |
| C3 | **No desktop-only defaults in the core.** The paths (`~/.local/share/lectrice`, `vendor/pdfium/...`, `ffmpeg`) and the TOML config stay in `lectrice-api`. Every core constructor takes explicit paths and numbers. Check: `grep` shows no `~`, `/usr`, `ffmpeg` or `.local` in domain/app/infra outside the `ffmpeg-export` module. | The phone passes `filesDir` paths. |
| C4 | **Registry wording:** `ExporterUnavailable` becomes "Audio export isn't available." (the ffmpeg detail goes in the log). Add device-neutral names (codes 4xxx): `ModelMissing` 4001, `ModelDownloadFailed` 4002, `ModelChecksumMismatch` 4003, `StorageFull` 4004, `ExportPaused` 4005 (operational, "Export paused; it will continue later."). | One registry for both apps (§4.6). |
| C5 | **Resumable export in parts** (§8.3): the export runner writes the book as numbered parts of about 10 listening-minutes each, records the finished parts in the export record, and on restart continues from the first unfinished part. The `AudioExporter` port gains `begin_part(target, part_index, sample_rate) -> ExportWriter` and `assemble(target, parts, chapters) -> ExportedFile`. `FfmpegExporter` implements it with WAV parts plus an ffmpeg concat. A `pause` (keep the parts) joins `cancel` (delete them). | Android 15+ stops long foreground work after 6 hours (§8.2). The desktop gains resume after a restart too. |
| C6 | `KokoroSpeechEngine` gets a **provider** setting (`"cpu"` default; `"xnnpack"` and `"nnapi"` to try, §15) and accepts a model dir **without `espeak-ng-data`** when built against sherpa-onnx 2.x (§14). | Spike measurements; the GPL question. |
| C7 | `PdfiumTextSource::new` takes a library **name or path** (it already takes `&Path`; document that a bare `libpdfium.so` means "let the system linker find it"). | On Android the native libs may stay inside the APK (§5.3). |
| C8 | Export `playback_order(document, read_skipped) -> Vec<SentenceId>` as a public service function (it exists in `read_ahead_plan.rs`). | The Media3 playlist uses the same order as the read-ahead, from one source. |
| C9 | `CHANGELOG.md`, `[workspace.package] version = "0.2.0"`, tag `v0.2.0`. | §3.4. |

---

## 4. Kotlin modules, packages and layers

### 4.1 Gradle modules (package root `app.lectrice`, proposed)

| Module | Layer | Contains | May depend on |
|---|---|---|---|
| `core:model` | **domain** | Pure Kotlin (no Android): `Document`, `Block`, `Sentence`, `TocEntry`, `DocumentSummary`, `Voice`, `AudioExport`, `PlaybackState`, `ErrorName` + `LectriceError`; the interfaces `LibraryRepository`, `DocumentRepository`, `VoiceRepository`, `PlaybackController`, `ExportScheduler`, `ModelInstaller`, `SettingsStore`; pure helpers ported from `frontend/src/utils` (`toc` tree / current chapter / short label, `timeLeft`, `printedLabel`, `playerStatus`) | nothing |
| `core:data` | **data** | `RustCore` (owns the uniffi `LectriceCore`), the repository implementations, record ↔ model mappers (one owner per mapping), error mapping, SAF import, `HttpModelInstaller`, DataStore settings | `core:model`, uniffi bindings, JNA |
| `core:playback` | data (platform) | `PlaybackService` (`MediaSessionService`), ExoPlayer setup, `SentenceDataSource`, the playlist window, `MediaControllerPlaybackController` (implements `PlaybackController` for the UI) | `core:model`, `core:data` (for `RustCore`) |
| `core:export` | data (platform) | `ExportWorker`, `PrepareWorker`, `ModelDownloadWorker`, `AacEncoderSink` (MediaCodec + MediaMuxer, implements the Rust `AudioEncoderSink`), the M4B chapter writer call | `core:model`, `core:data` |
| `core:designsystem` | **ui** | `LectriceTheme` (tokens, §10), components: `PillPlayer`, `SheetScaffold`, `PageChip`, `ChapterRow`, `ProgressLine`, `StatusBanner`, icons | Compose only |
| `feature:library` | **ui** | `LibraryScreen` + `LibraryViewModel` | `core:model`, `core:designsystem` |
| `feature:reader` | **ui** | `ReaderScreen`, `PlayerSheet`, `ChaptersSheet` + `ReaderViewModel`, `ChaptersViewModel` | `core:model`, `core:designsystem` |
| `feature:settings` | **ui** | `SettingsScreen`, `ModelSetupScreen`, `LicensesScreen` + ViewModels | `core:model`, `core:designsystem` |
| `app` | composition root | `MainActivity`, `LectriceApplication`, `AppContainer` (builds everything by hand; no DI framework), the navigator | everything |

**Feature modules never see `core:data`, Media3 or uniffi types.** They get interfaces from `core:model`, and the
`AppContainer` hands them in through ViewModel factories. Tests and Compose previews use fakes of the same interfaces (like
`frontend/src/api/mock`).

### 4.2 The rules, mapped to Android

| Backend rules / web rules | Android |
|---|---|
| Repository (only it talks to the outside) / `api/` | `core:data` repositories: the only code that calls `RustCore`, the file system, the network or DataStore |
| Service / use case / custom hook | **ViewModel**: holds screen state as `StateFlow<UiState>`, calls repositories, exposes actions (`play()`, `jumpTo(id)`). Returns data, never UI. |
| Mapper / pure `utils/` | Pure Kotlin functions in `core:model` (`toc.kt`, `timeLeft.kt`), unit-tested on the JVM |
| Controller / page | The **screen** composable: collects the ViewModel state (`collectAsStateWithLifecycle()`) and wires events. Thin. |
| Component | A **stateless composable**: parameters in, lambdas out (state hoisting = props down, events up) |
| View state stays in the view | `rememberSaveable` in the composable (a sheet's open state, the expanded chapter, follow-scroll on/off) |
| "Pass the difference in" (slots) | Composable slot parameters (`content: @Composable () -> Unit`) instead of mode flags |
| Map, don't chain ternaries | `when` over a sealed/enum type, which the compiler checks for completeness |
| One error registry | §4.6 |
| Structured logging at call boundaries, never book content | `RustCore` logs each call (name, ids, counts, duration) through a small `Log` wrapper; the Rust side logs through `LogSink` (§6.5). Never sentence text, titles of the user's books, or PDF names in release logs. |
| No magic values | `object` constants and `enum class` (`PlaybackSpeeds`, `ReadAheadDefaults`) |
| DI | Constructor injection, by hand, in `AppContainer` |

### 4.3 Data flow

`RustCore` → repository (`core:data`) → ViewModel `StateFlow` → screen → components. Events flow back up. Playback state comes
from the `MediaController` (in `core:playback`), adapted to the `PlaybackController` interface, so the reader never touches
Media3.

### 4.4 Threading

- **No core call on the main thread.** Every `RustCore` method is `suspend` and runs `withContext(Dispatchers.IO)`. In debug
  builds `RustCore` checks `Looper.myLooper() != Looper.getMainLooper()` and throws, because StrictMode doesn't see JNA calls.
- The speech worker and the export jobs are **Rust threads** (as on the desktop). Kotlin never runs synthesis on its own threads.
- `SentenceDataSource` blocks on ExoPlayer's loader thread, which is a background thread made for that.

### 4.5 Navigation (Telegram's "screens we control")

One `MainActivity`. A small navigator of our own (proposed, about 100 lines): a back stack of
`sealed interface Screen { Library; Reader(docId); Settings; ModelSetup; Licenses }`, saved with `rememberSaveable`. It shows
the top screen with `AnimatedContent` plus spring transitions, and handles the system back gesture with `PredictiveBackHandler`,
so the screen follows the finger. The two sheets (player, chapters) are not screens; they are state inside the reader.
Navigation 3 (where you also own the back stack) is the alternative if we later need more; its stable version is UNVERIFIED.

### 4.6 One error registry

- **Source of truth: the Rust registry** (`lectrice-domain/src/error.rs`), including the new 4xxx names (C4).
- `CoreError` crosses uniffi with `name`, `code`, `operational` and `user_message` (the registry's description, never book text).
- Kotlin has `enum class ErrorName(val code: Int)` in `core:model`, with the same names and codes, and a `strings.xml` entry
  per name for the user-facing text (so it can be translated later). `core:data` maps a `CoreException` to
  `LectriceError(name)`. Errors that start in Kotlin (download, SAF copy, storage full) use the same enum.
- **A test keeps them equal:** `ErrorRegistryTest` (JVM, §13) loads the host build of `liblectrice_ffi` and compares
  `errorRegistry()` with `ErrorName.entries`.

---

## 5. Rust: the `lectrice-ffi` crate (Android repo)

### 5.1 Crate

```toml
[package]  name = "lectrice-ffi"   edition = "2024"
[lib]      crate-type = ["cdylib", "lib"]   name = "lectrice_ffi"
[[bin]]    name = "uniffi-bindgen"   path = "src/bin/uniffi-bindgen.rs"   # generates the Kotlin
[dependencies]  # core crates as in §3.2, plus:
uniffi = { version = "=0.32.2", features = ["cli"] }
tracing = "=0.1.44"
tracing-subscriber = { version = "=0.3.23", default-features = false, features = ["registry", "std"] }
```

Modules: `api.rs` (the exported object), `records.rs` (uniffi records and mappers from domain types), `errors.rs`,
`config.rs`, `composition.rs` (calls `lectrice_app::services::assemble`), `export_sink.rs` (the `AudioExporter` adapter that
forwards to Kotlin), `log_bridge.rs` (a `tracing` layer that forwards to `LogSink`), `bench.rs` (spike only, behind feature
`bench`).

### 5.2 API surface (uniffi proc-macros)

```rust
// ---------- config ----------
#[derive(uniffi::Record)] pub struct CoreConfig {
    pub data_dir: String,                 // context.filesDir + "/lectrice"
    pub pdfium_library: String,           // "libpdfium.so" (bare name; §5.3)
    pub engine: EngineConfig,
    pub default_voice: String,            // "af_heart"
    pub read_ahead_minutes: u32,          // 10
    pub keep_minutes_per_document: u32,   // 60
    pub cache_max_mb: u64,                // 1024 on phones (proposed; 2048 on desktop)
    pub import_max_mb: u64,               // 200
}
#[derive(uniffi::Enum)] pub enum EngineConfig {
    Kokoro { model_dir: String, num_threads: u32, provider: String },   // "cpu" (default) | "xnnpack" | "nnapi" (§15)
}
#[derive(uniffi::Enum)] pub enum EngineStatus { Ready { engine_id: String }, Missing, Failed { error: CoreErrorInfo } }

// ---------- records (camelCase in Kotlin; ids are plain numbers/strings) ----------
#[derive(uniffi::Record)] pub struct DocumentSummaryRecord { pub id: String, pub title: String, pub source_file_name: String,
    pub page_count: u32, pub sentence_count: u32, pub added_at_epoch_s: i64, pub last_opened_at_epoch_s: i64, pub last_sentence_id: Option<u32> }
#[derive(uniffi::Record)] pub struct ImportResult { pub summary: DocumentSummaryRecord, pub already_in_library: bool, pub duration_ms: u64 }
#[derive(uniffi::Record)] pub struct SummaryPage { pub items: Vec<DocumentSummaryRecord>, pub total: u32, pub limit: u32, pub offset: u32 }
#[derive(uniffi::Record)] pub struct DocumentRecord { pub id: String, pub title: String, pub page_count: u32, pub cleaner_version: u32,
    pub pages: Vec<PageLabelRecord>, pub toc: Vec<TocEntryRecord>, pub blocks: Vec<BlockRecord>, pub last_sentence_id: Option<u32> }
#[derive(uniffi::Record)] pub struct PageLabelRecord { pub index: u32, pub label: String }
#[derive(uniffi::Record)] pub struct TocEntryRecord { pub id: u32, pub title: String, pub level: u8, pub sentence_id: u32, pub page_index: u32 }
#[derive(uniffi::Enum)]   pub enum BlockKind { Heading, Paragraph, ListItem, Note, Code, Table, Caption }
#[derive(uniffi::Record)] pub struct BlockRecord { pub id: u32, pub kind: BlockKind, pub level: Option<u8>, pub page_index: u32,
    pub spoken: bool, pub text: Option<String>, pub sentences: Vec<SentenceRecord>, pub cue: Option<SentenceRecord> }
#[derive(uniffi::Record)] pub struct SentenceRecord { pub id: u32, pub text: String, pub page_index: u32 }   // speech_text stays in Rust
#[derive(uniffi::Enum)]   pub enum VoiceLanguage { EnUs, EnGb }
#[derive(uniffi::Enum)]   pub enum VoiceGender { Female, Male }
#[derive(uniffi::Record)] pub struct VoiceRecord { pub id: String, pub name: String, pub language: VoiceLanguage, pub gender: VoiceGender }
#[derive(uniffi::Record)] pub struct VoiceList { pub voices: Vec<VoiceRecord>, pub default_voice_id: String }
#[derive(uniffi::Record)] pub struct SpeakArgs { pub document_id: String, pub sentence_id: u32, pub voice_id: String, pub rate: f32, pub read_skipped: bool }
#[derive(uniffi::Record)] pub struct SpokenAudio { pub wav: Vec<u8>, pub duration_ms: u32, pub cache_hit: bool }
#[derive(uniffi::Enum)]   pub enum ExportFormat { M4b }                       // Mp3 stays desktop-only
#[derive(uniffi::Enum)]   pub enum ExportState { Queued, Running, Paused, Done, Failed, Cancelled }
#[derive(uniffi::Record)] pub struct AudioExportRecord { pub id: String, pub document_id: String, pub voice_id: String, pub format: ExportFormat,
    pub state: ExportState, pub sentences_done: u32, pub sentences_total: u32, pub parts_done: u32, pub parts_total: u32,
    pub estimated_seconds_left: Option<u32>, pub file_path: Option<String>, pub error: Option<CoreErrorInfo> }

// ---------- errors ----------
#[derive(uniffi::Record)] pub struct CoreErrorInfo { pub name: String, pub code: u32, pub operational: bool, pub user_message: String }
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum CoreError { #[error("{name}")] Failed { name: String, code: u32, operational: bool, user_message: String } }  // Kotlin: CoreException.Failed
// `user_message`, not `message`: uniffi makes CoreError a Kotlin exception, and a `message` field clashes with Throwable.message (found in S3).

// ---------- callbacks (Kotlin implements) ----------
#[uniffi::export(with_foreign)] pub trait LogSink: Send + Sync {
    fn log(&self, level: LogLevel, target: String, message: String, fields_json: String);    // ids, counts, durations only
}
#[uniffi::export(with_foreign)] pub trait CoreListener: Send + Sync {
    fn on_export_changed(&self, export: AudioExportRecord);   // at most every 5 % and on every state change
}
#[uniffi::export(with_foreign)] pub trait AudioEncoderSink: Send + Sync {   // MediaCodec AAC in Kotlin (§8.4)
    fn begin_part(&self, part_path: String, sample_rate: u32) -> Result<u64, CoreError>;       // returns a handle
    fn append(&self, handle: u64, pcm16: Vec<i16>) -> Result<(), CoreError>;                    // one sentence at a time
    fn finish_part(&self, handle: u64) -> Result<(), CoreError>;
    fn abandon_part(&self, handle: u64);
    fn assemble(&self, part_paths: Vec<String>, chapters: Vec<ChapterMark>, target_path: String, title: String) -> Result<u64, CoreError>; // bytes
}
#[derive(uniffi::Record)] pub struct ChapterMark { pub title: String, pub start_ms: u64 }

// ---------- the object ----------
#[derive(uniffi::Object)] pub struct LectriceCore { /* Arc<Services>, RwLock<Option<speech + export services>> */ }

#[uniffi::export] impl LectriceCore {
    #[uniffi::constructor]
    pub fn new(config: CoreConfig, log: Arc<dyn LogSink>, listener: Arc<dyn CoreListener>, encoder: Arc<dyn AudioEncoderSink>) -> Result<Arc<Self>, CoreError>;
    pub fn engine_status(&self) -> EngineStatus;
    pub fn reload_engine(&self, engine: EngineConfig) -> EngineStatus;   // after a model download or a settings change; also "unload" when idle (§5.4)
    pub fn unload_engine(&self);

    pub fn import_pdf(&self, pdf_path: String, file_name: String) -> Result<ImportResult, CoreError>;
    pub fn list_recent(&self, limit: u32, offset: u32) -> Result<SummaryPage, CoreError>;
    pub fn get_document(&self, document_id: String) -> Result<DocumentRecord, CoreError>;
    pub fn playback_order(&self, document_id: String, read_skipped: bool) -> Result<Vec<u32>, CoreError>;
    pub fn save_position(&self, document_id: String, sentence_id: u32) -> Result<(), CoreError>;

    pub fn list_voices(&self) -> Result<VoiceList, CoreError>;
    pub fn speak_sentence(&self, args: SpeakArgs) -> Result<SpokenAudio, CoreError>;   // blocks until the clip is ready; moves read-ahead
    pub fn stop_read_ahead(&self, document_id: String);                               // on pause > 1 min, on stop, on low memory

    pub fn start_export(&self, document_id: String, voice_id: String, format: ExportFormat, read_skipped: bool) -> Result<AudioExportRecord, CoreError>;
    pub fn resume_exports(&self) -> Vec<AudioExportRecord>;     // the worker calls this when it starts
    pub fn pause_export(&self, export_id: String) -> Result<AudioExportRecord, CoreError>;   // keeps finished parts
    pub fn cancel_export(&self, export_id: String) -> Result<(), CoreError>;                 // deletes parts
    pub fn get_export(&self, export_id: String) -> Result<AudioExportRecord, CoreError>;
    pub fn list_exports(&self, document_id: String) -> Result<Vec<AudioExportRecord>, CoreError>;

    pub fn cache_usage_mb(&self) -> u64;
    pub fn clear_audio_cache(&self) -> Result<(), CoreError>;
}

#[uniffi::export] pub fn error_registry() -> Vec<CoreErrorInfo>;   // for the Kotlin mirror test
#[uniffi::export] pub fn core_version() -> String;                  // the core tag, shown in Settings → About
```

### 5.3 Boundary rules

- **Coarse calls.** JNA costs microseconds per call: fine per sentence, never per word or sample.
- **Blocking functions; Kotlin picks the thread.** No uniffi async or tokio at first (fewer moving parts).
- **Callbacks come from Rust threads** (speech worker, export jobs). The Kotlin side only copies the record into a
  `MutableStateFlow` and returns. JNA attaches the thread to the JVM for each callback; if profiling shows a cost, set a
  `CallbackThreadInitializer` to keep it attached (UNVERIFIED that it's needed).
- **Sentence audio comes back as WAV bytes** (`SpokenAudio.wav`, about 48 KB per second, so 100–400 KB per sentence). That's
  one copy per sentence and needs no core change. Returning a cache file path instead is a later optimisation if the spike
  shows the copy matters (UNVERIFIED that it does).
- **`get_document` for a 400-page book** is a few MB of records. The spike measures the mapping time. If it takes more than
  about 1 s, add `get_document_blocks(id, from_block, count)` paging (UNVERIFIED).
- **Panics** become `InternalException` in Kotlin (uniffi catches them). A C++ abort inside sherpa or pdfium still kills the
  process; the engine checks from web spec §10 (paths, NUL bytes, voice ids) stay. sherpa-onnx 2.0's C API now catches C++
  exceptions in its `Create*` functions.
- **Errors never carry book text.** `detail` and the cause chain go to `LogSink` only, at `warn`/`error`.

### 5.4 Engine lifetime and memory

- The engine loads lazily: on the first play, a Prepare job or an export (not at app start). Then it stays loaded while
  anything needs it.
- `unload_engine()` runs after 5 minutes with no playback or job, and on `onTrimMemory(TRIM_MEMORY_BACKGROUND)` or higher
  (proposed). Kokoro was reported at about 830 MB on iOS; on Android it is UNVERIFIED (the spike measures it).
- One engine instance, one speech worker thread, so one synthesis runs at a time. Its priorities stay as on the desktop:
  now-playing > read-ahead > prepare/export.

### 5.5 Paths on Android

| What | Path (all private app storage; no permission needed) |
|---|---|
| Library, audio cache, export records and parts | `filesDir/lectrice/` (the same layout as web spec §6 under `data_dir`) |
| Model | `filesDir/models/<bundle-id>/` (e.g. `kokoro-en-v1_0-fp32/`) |
| Imported PDF before cleaning | `cacheDir/import/<uuid>.pdf`, deleted after `import_pdf` |
| pdfium | `"libpdfium.so"`: a bare name, so `dlopen` searches the app's native lib dir, which also works when the libs stay uncompressed inside the APK (UNVERIFIED; the spike tests it). Fallback: `applicationInfo.nativeLibraryDir + "/libpdfium.so"` with `packaging.jniLibs.useLegacyPackaging = true`. |
| sherpa + onnxruntime | Packaged in `jniLibs/arm64-v8a/` next to `liblectrice_ffi.so`. JNA loads `liblectrice_ffi.so`, and the system linker resolves `libsherpa-onnx-c-api.so` → `libonnxruntime.so`. |

### 5.6 Build

- `cargo ndk -t arm64-v8a --platform 29 -o <module>/src/main/jniLibs build -p lectrice-ffi --release`
- then `cargo run -p lectrice-ffi --bin uniffi-bindgen -- generate --library target/aarch64-linux-android/release/liblectrice_ffi.so --language kotlin --out-dir core/data/build/generated/uniffi`
- A Gradle task in `core:data` runs both before `preBuild`. `native/fetch-libs.sh` downloads the pinned sherpa-onnx Android
  archive and `pdfium-android-arm64.tgz` (`chromium/7881`), checks `native/SHA256SUMS`, and copies the three `.so` files into
  the gitignored `jniLibs/`. `SHERPA_ONNX_LIB_DIR` points `sherpa-onnx-sys` at the same folder, so it doesn't download a second copy.
- **Binaries are never committed** (the public repo stays small; the checksums keep builds reproducible).
- A host build (`cargo build -p lectrice-ffi`, x86_64 Linux, static sherpa) serves the JVM tests (§13).
- Release: `[profile.release] lto = "thin"`, `codegen-units = 1`, `strip = "symbols"`, `panic = "unwind"` (uniffi needs unwind
  to catch panics).

---

## 6. Model download on first run

### 6.1 Which bundle: the user chooses (decided 2026-10-23)

The first-run screen offers **both** bundles, each with its size and a one-line note on quality and speed. The user can switch
later in Settings: download the other one, then delete the old one.

| Option | Size (download) | Speed on phones | Quality |
|---|---|---|---|
| **Full quality (fp32)** Kokoro v1.0, our desktop model | about 370 MB (about 350 MB once sherpa 2.0 drops `espeak-ng-data`) | S10 (Exynos): RTF 1.13 at 4 threads (published; a different Kokoro version) | the reference; the user approved it |
| **Smaller (int8)** Kokoro multi-lang v1.0 | about 126 MB | S10: RTF 2.42, i.e. **slower** (published). On newer ARM cores with int8 dot-product instructions it may be faster (UNVERIFIED) | slightly lower (UNVERIFIED by ear) |

- **The notes on the screen come from the spike's own numbers** (plan §5.3), not from guesses. For example: "Full quality ·
  370 MB · real-time on recent phones" and "Smaller · 126 MB · a little less natural · slower on most phones". The final
  wording is written after M1.
- The screen pre-selects nothing; it shows one sentence on what to pick when unsure (proposed: "Not sure? Full quality.").
- Each bundle has its own `bundleId`, so the audio cache keys (engine id) differ. Switching bundles never plays a clip that
  was made by the other one.
- Settings → Voice model: shows the installed bundle(s) and their sizes, "Download the other one", and "Delete" (only for the
  one not in use).

### 6.2 What we ship, and from where

- **Our own trimmed bundle** (proposed), published as a release asset of the `lectrice-android` repo: `model.onnx`,
  `voices.bin`, `tokens.txt`, `lexicon-us-en.txt`, `lexicon-gb-en.txt`, plus `espeak-ng-data/` (only while we use sherpa-onnx
  1.x, §14), and `LICENSE`/`NOTICE` for Kokoro (Apache-2.0). The Chinese files are dropped.
- **Single files, not an archive.** No bzip2 or tar code is needed on the phone, each file resumes on its own, and `.onnx`
  barely compresses anyway.
- A **manifest** is compiled into the app: `bundleId`, and per file the URL, size and SHA-256. The app trusts only the hashes it
  ships with.

### 6.3 How it downloads

- `ModelDownloadWorker` (WorkManager, foreground type `dataSync`, a progress notification). Constraint: network connected. The
  first screen offers the two bundles (§6.1) with "Wi-Fi recommended". It warns on a metered network but doesn't block.
- Plain `HttpURLConnection` (no OkHttp: fewer dependencies). It follows the GitHub redirect and **resumes with `Range`** from
  the size of the `.part` file. Range support on GitHub's asset hosts is UNVERIFIED; if a server ignores `Range` (it answers
  200, not 206), the file restarts from zero.
- Each file is hashed while it streams (`MessageDigest`). After the last byte: compare to the manifest. On a match, rename
  `.part` → final. On a mismatch, delete it, report `ModelChecksumMismatch`, and retry once.
- When all files are in, `reload_engine(Kokoro { model_dir, … })`. `engine_status()` must become `Ready`.
- Before starting: `StorageManager.getAllocatableBytes` must show model size + 200 MB free, else `StorageFull`.
- **"Import model from file…"** (SAF, a `.zip` of the same files, checked against the same manifest) for offline installs. It
  uses `java.util.zip`, which is built in.
- The model sits in `filesDir`, so "Clear cache" keeps it and "Clear storage" removes it. It is excluded from Auto Backup
  (`dataExtractionRules`), and so is the audio cache.

---

## 7. Playback with Media3

### 7.1 Pieces

- **`PlaybackService : MediaSessionService`**, holding one `ExoPlayer` and one `MediaSession`. Manifest:
  `android:foregroundServiceType="mediaPlayback"`, permissions `FOREGROUND_SERVICE`, `FOREGROUND_SERVICE_MEDIA_PLAYBACK`,
  `POST_NOTIFICATIONS` (asked for the first time the user presses play, API 33+), `WAKE_LOCK`.
- `ExoPlayer` setup: `AudioAttributes(USAGE_MEDIA, AUDIO_CONTENT_TYPE_SPEECH)` with `handleAudioFocus = true`,
  `setHandleAudioBecomingNoisy(true)` (unplugging headphones pauses), and **`setWakeMode(C.WAKE_MODE_LOCAL)`**. That last
  one keeps the CPU awake while playing with the screen off; synthesis needs it.
- The UI connects with a `MediaController` (async `SessionToken`). `MediaControllerPlaybackController` adapts it to the
  `PlaybackController` interface (`state: StateFlow<PlaybackState>`, `play()`, `pause()`, `next()`, `previous()`, `jumpTo(id)`,
  `setSpeed(r)`, `setVoice(id)`, `setReadSkipped(b)`).
- **Lock screen, notification, Bluetooth and headset buttons come free** with the session's default notification.
  Next/previous mean the next/previous **sentence**, the same as the pill (user decision 2026-10-23). The notification shows the book title, and the
  current chapter as the subtitle.

### 7.2 The playlist of sentences

- One `MediaItem` per spoken sentence (or cue). `mediaId = "<docId>:<sentenceId>"`,
  `uri = lectrice://sentence/<docId>/<sentenceId>`.
- **A sliding window, not the whole book** (proposed): the player holds about 200 items around the current one, taken from
  `playback_order(docId, readSkipped)`. When fewer than 50 remain ahead, the next 200 are appended and the oldest ones dropped.
  A jump, a voice change or a "read code & tables" change rebuilds the window around the target. Thousands of items would
  make every timeline change slow (UNVERIFIED how slow; the window avoids the question).
- Position: `onMediaItemTransition` → the sentence id → `PlaybackState.currentSentenceId` → the reader's highlight. The
  position is saved through `save_position` at most every 5 s and on pause or stop (proposed debounce).
- **Seek bar:** per-sentence progress is useless, so the session removes `COMMAND_SEEK_IN_CURRENT_MEDIA_ITEM`. Whether One UI
  then hides the bar is UNVERIFIED. Book-level progress lives in the app's player sheet.
- **Process death / resume:** `MediaSession.Callback.onPlaybackResumption` rebuilds the window from the saved position, so
  "play" from the system media controls continues the last book.

### 7.3 The custom DataSource

`SentenceDataSource : BaseDataSource(isNetwork = false)`:

1. `open(dataSpec)`: parse the URI, read the current speed / voice / readSkipped from `PlaybackSettingsStore`, call
   `rustCore.speakSentenceBlocking(SpeakArgs(...))`, keep the `ByteArray`, and return its length.
2. `read()` copies from the array. `close()` drops it.

It sits under `ProgressiveMediaSource.Factory(SentenceDataSource.Factory(...))`, and the default extractors read the WAV.
While `open` waits for synthesis, the player is `BUFFERING`, so the pill shows "Preparing audio…".

- ExoPlayer buffers the next items by itself (that's the near prefetch). Rust read-ahead keeps about 10 listening-minutes ready
  in the cache (scaled by speed, as on the desktop).
- A blocking call can't be interrupted. After a jump, the old sentence still finishes and lands in the cache. That costs at most
  one sentence of work (accepted).

### 7.4 Speed and highlight

- Speed: `player.setPlaybackParameters(PlaybackParameters(rate))`, which keeps the pitch. The audio is always made at 1×, so
  the cache stays valid; `rate` only sizes the read-ahead (as on the desktop).
- The highlight follows `onMediaItemTransition`. A sentence is the unit, as on the web; there is no word-level highlight in v1.

---

## 8. Read-ahead, "Prepare" and export (background work)

### 8.1 What runs where

| Job | Who keeps the process alive | Engine priority |
|---|---|---|
| Read-ahead while listening | `PlaybackService` (foreground `mediaPlayback`) | 2 |
| **Prepare** a chapter or the book ahead of time (**only if the spike shows it's needed**, §8.5) | `PrepareWorker` (WorkManager) | 3 |
| Export M4B | `ExportWorker` (WorkManager) | 3 |

Read-ahead stops when playback pauses for more than 1 minute (proposed), so a paused phone doesn't keep synthesising.

### 8.2 The Android 15+ six-hour limit

On Android 15 and newer, foreground services of type `dataSync` or `mediaProcessing` may run **6 hours in total per 24 h**.
After that the system calls `Service.onTimeout()` and the work must stop. The budget resets when the user brings the app to
the foreground. An 8-hour book at RTF ≈ 1 needs about 8 hours, **so export and Prepare must be resumable.** (The S10+ is on
Android 12, so it doesn't have this limit; the S26 Ultra does.)

### 8.3 How it resumes

- The core writes the export as **parts of about 10 listening-minutes** (C5). Each finished part is recorded in the export
  record on disk. Restarting continues at the next unfinished part, so at most one part's work is lost.
- `ExportWorker : CoroutineWorker`:
  1. Calls `setForeground(...)` with type `mediaProcessing` on API 35+ (`dataSync` below that), and a notification
     "Exporting <title>: 42 %". The worker shows the title on the device only; it never logs it.
  2. Calls `resume_exports()`, then waits on `CoreListener` updates until the job is `Done`, `Failed` or `Cancelled`.
  3. When stopped (`onStopped`, including `STOP_REASON_FOREGROUND_SERVICE_TIMEOUT`), it calls `pause_export(id)` and returns.
  4. After a timeout it re-enqueues itself as **regular, non-foreground work** with "charging" and "battery not low"
     constraints. Each such run works for up to 9 minutes, pauses, and re-enqueues. The next time the app opens, it is promoted
     back to foreground work (proposed).
- Constraints: Export runs without constraints when the user starts it. Prepare defaults to "while charging" (proposed).
- Samsung's battery optimisation may kill background work earlier (UNVERIFIED per device). Settings links to the system's
  "unrestricted battery" page with a one-line explanation, but doesn't nag.

### 8.4 Encoding (Kotlin side of the exporter port)

- `AacEncoderSink` implements the Rust `AudioEncoderSink`: `MediaCodec` AAC-LC, mono, 24 kHz, 64 kbit/s, into one `.m4a` per
  part with `MediaMuxer`.
- `assemble`: remux every part into one `.m4b` (`MediaExtractor` → `MediaMuxer`, no re-encoding), then add chapter marks.
  **MediaMuxer can't write chapters**, so a small step adds a Nero `chpl` box (plus, if needed, a QuickTime chapter track) to
  the finished file. Whether the common players (Smart AudioBook Player, VLC, Apple Books) read `chpl` written this way is
  UNVERIFIED; that's an M4 task with a test file.
- **Where the file goes:** written to `filesDir/lectrice/exports/...`, then offered as "Save to Audiobooks" (MediaStore,
  `Audiobooks/Lectrice/`, so audiobook apps find it) or "Save as…" (SAF `CreateDocument("audio/mp4")`), and "Share".

### 8.5 Prepare mode (only if the spike shows it's needed; user 2026-10-23)

It is built **only if** the spike puts a phone the user cares about in the amber or red band (plan §6). Otherwise read-ahead
while listening is enough, and Prepare is left out. If it is built: the same plan as export, but the clips go into the **audio cache**, not into a file, and the cache window grows for that book
while it is prepared (proposed: up to the global cap). Listening then plays straight from the cache. This is the main answer if
the phone is slower than real time (spike gate, plan §6).

---

## 9. File import (SAF)

- "Open PDF" uses `rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument())` with `arrayOf("application/pdf")`.
- Also an intent filter for `ACTION_VIEW` and `ACTION_SEND` with `application/pdf` ("Open with Lectrice" / "Share to
  Lectrice") (proposed).
- `LibraryRepository.import(uri)`:
  1. Read `OpenableColumns.DISPLAY_NAME` and `SIZE`.
  2. Stream the content into `cacheDir/import/<uuid>.pdf`. Stop with `UploadTooLarge` past `import_max_mb`, and with
     `StorageFull` if writing fails for lack of space.
  3. Call `import_pdf(path, displayName)`. Delete the temp file in `finally`.
- No persistable URI permission is kept: the core saves its own copy of the original (as on the desktop).
- The UI shows "Reading… cleaning text" (indeterminate) during the import. If the spike's largest-book import takes longer than
  10 s on the S10+, M2 adds page progress (a core port change, so it would go into a core release) (proposed).
- Errors use the registry: `PdfInvalid`, `PdfEncrypted`, `PdfNoText`, `UploadTooLarge`, `StorageFull`.

---

## 10. Theme tokens (ported from `frontend/src/styles/tokens.css`)

`core:designsystem/LectriceTheme.kt`: an immutable `LectriceColors` data class plus `LectriceType` and `LectriceSpacing`,
given through `CompositionLocal`s (`LectriceTheme.colors.hl`), like CSS variables. Material 3's `MaterialTheme` is set from the
same values, so the few M3 widgets we use (slider, switch, bottom sheet) match.

| Token | Light | Dark | Use |
|---|---|---|---|
| `bg` | `#FFFFFF` | `#121417` | screen background |
| `bg2` | `#F3F3EF` | `#1C1F24` | pill, sheets, chips |
| `fg` | `#1C1F24` | `#E8E6E1` | text |
| `fg2` | `#5B636D` | `#A8ADB5` | secondary text (5.47:1 on `bg2` light, 7.33 dark; 6.09 on `bg`) |
| `line` | `#D3D3CD` | `#363A41` | borders, dividers |
| `accent` | `#1D5FAE` | `#8AB8FF` | play button, current chapter bar, focus |
| `onAccent` | `#FFFFFF` | `#0B1A33` | icon on the play button |
| `hl` | `#FFE58F` | `#4D4214` | current-sentence background |
| `hlFg` | `#1C1F24` | `#FFF6D8` | current-sentence text |
| `errBg` | `#FBE7E5` | `#44181A` | error banner |
| `errFg` | `#8B1A12` | `#F4B1AD` | error banner text |

- **Type:** reading text uses the serif at 19 sp with a 1.7 line height (32 sp), from the web's 1.1875 rem / 1.7. UI text
  uses the system sans; page chips use sans 12 sp in `fg2`. Charter isn't on Android, so v1 uses `FontFamily.Serif` (Noto
  Serif on most phones). Bundling an OFL serif (e.g. Source Serif 4, ≈ 300 KB) is an option if the user doesn't like Noto
  Serif (proposed).
- Font sizes follow the system font scale (`sp`). The reader adds its own text-size control in Settings (proposed).
- Light/dark follows the system by default, with an override in Settings. **No dynamic (wallpaper) colour**: the colours are
  ours.
- Contrast is measured, not guessed: a JVM unit test computes the contrast of every text/background pair above (≥ 4.5:1) and
  fails the build if one drops.
- Spacing tokens come from the web layout: 16 dp phone gutter, pill height 56 dp, button 36/44 dp → **48 dp touch targets** on
  Android (the visual size can stay smaller).

---

## 11. Screens

All screens are edge-to-edge (enforced from targetSdk 35) and handle insets.

### 11.1 Library

- Top: app name and a Settings icon. A large **"Open PDF"** button.
- "Recent" list (`LazyColumn`): title, "p. 37 of 412 · 9 %", last opened; tap = open the reader at the saved position
  ("Continue"). Paged 20 at a time from `list_recent`.
- During an import: a row with "Reading… cleaning text". When done, the reader opens. If the book was already in the library, a
  short note says so.
- Empty state: one line on what Lectrice does, plus the button.
- If the model isn't installed: a banner "Voice not installed. Download (350 MB)" → Model setup. Reading still works without the
  voice.

### 11.2 Reader (design D on a phone)

- Full-screen text, 16 dp side gutter, no top bar.
- `LazyColumn`, one item per block, `key = block.id`. A paragraph is one `Text` with an `AnnotatedString`. The current sentence
  gets the `hl`/`hlFg` span. Only the block holding the current sentence recomposes (its index comes from a `derivedStateOf`).
- Page markers are **inline chips** ("p. 37") at the sentence where the page changes, as on the web's phone layout. They use
  `InlineTextContent`, are not selectable, and are never spoken.
- Code blocks: monospace, horizontal scroll inside the block. Tables and captions are shown as text.
- **Tap a sentence = jump there.** The tap position maps to a sentence through `TextLayoutResult.getOffsetForPosition`. A jump
  keeps the play/pause state.
- **Follow-scroll:** keep the current sentence in view until the user scrolls. Then a "locate" button appears in the pill.
- **The pill** (bottom, full width minus 16 dp each side): previous · play/pause (accent) · next · status · locate (when
  follow-scroll is off) · chapters · more. Status on a phone: `Ch. 7 · p. 412` (short chapter label + page, chapters brief §3/§4
  rules). It is hidden below 360 dp width (proposed; the web hides it at 420 px).
- Volume keys control media volume while the reader is open (`setVolumeControlStream`).

### 11.3 Player sheet (the web's "more" popover)

A bottom sheet over the reader, with the pill still visible above it. It holds:

- a book progress slider + "12 min left";
- the chapter line (`7. Long Documents › 7.3 …`, 2 lines max) + "38 % of this chapter · 9 min left in it";
- the location line `Page 412 · PDF page 425 of 589` and a "Chapters…" button;
- speed (0.75–2× chips), voice (a list grouped US/UK, female/male), "Read code & tables aloud";
- the export panel (start → progress → "Save to Audiobooks" / "Save as…" / "Share", plus pause/cancel);
- "Open another PDF…".

Drag to dismiss, back to close.

### 11.4 Chapters sheet (bottom-sheet form, chapters brief option 3)

- Opened from the pill's chapters button or "Chapters…". It covers 85 % of the height, is modal on phones, and has rounded top
  corners.
- **One level at a time:** the chapter list; a "›" on a chapter drills into its sections (the header gets "‹ All chapters").
  Tapping a title jumps there, keeps play/pause, resumes follow-scroll, and closes the sheet.
- Rows: the title (2 lines max), the printed page label on the right, and progress (`read` = check mark + `fg2`, `current` =
  3 dp accent bar + semibold + "38 %", `unread` = plain). Collapsed chapters show "6 sections".
- On open, the current row is scrolled into the centre.
- A flat book (all level 1) has no "›". An empty TOC shows the brief's empty-state text.
- TalkBack: each row reads "title, page 412, current"; the drill-in button reads "Show sections of …".
- The label style A/B/C and the "current chapter" rule are still open on the web side; the phone uses the same pure helpers
  (`core:model/toc.kt`, ported from `frontend/src/utils/toc.ts`), so a decision there applies to both.

### 11.5 Settings and Model setup

- **Model setup** (first run, or from the banner): what the voice is, the size, "Download" / "Import from file…", then
  progress (MB, %, speed, "Paused, waiting for network"), then a "Hear a sample" button.
- **Settings:** voice model (installed bundle, size, delete/re-download; fp32/int8 if both are offered), default voice and
  speed, read-ahead minutes (5/10/20), audio cache size and "Clear audio cache", theme (system/light/dark), text size, battery
  note (§8.3), "Open-source licenses", "About" (app version + `core_version()`).

---

## 12. Telegram principles, concretely

| Principle | What it means in Lectrice |
|---|---|
| One Activity, screens we control | One `MainActivity`; our own back stack of `Screen`s (§4.5) with spring transitions and predictive back that follows the finger. Sheets are reader state, not new screens. |
| Our own small theme engine | `LectriceTheme` with the 11 tokens above, light/dark, no wallpaper colours. Every component reads tokens; no hard-coded colours outside the theme file (a lint check: grep for `Color(0x` outside `designsystem`). |
| Custom drawing only where profiling shows a need | Start with `Text` + `AnnotatedString`. Only if the reader benchmark shows jank (§13) does the paragraph switch to `Canvas` + `TextMeasurer` with layouts cached per block. That's the Compose version of Telegram's hand-drawn `ChatMessageCell`, used in one place. |
| Nothing heavy on the UI thread | All core calls are on `Dispatchers.IO` (checked in debug). Synthesis is on the Rust worker thread. The document → UI model mapping runs off the main thread, once per open. StrictMode is on in debug builds. |
| Few dependencies | Compose (+ Material 3 for a few widgets), Activity, Lifecycle/ViewModel, Media3 (exoplayer + session), WorkManager, DataStore, coroutines, JNA. No DI framework, no navigation library, no OkHttp, no image library, no analytics. Each new dependency needs a written reason in the PR. |
| Tuned spring and gesture animations | `spring(dampingRatio = 0.85f, stiffness = Spring.StiffnessMediumLow)` as the default (proposed starting values, tuned on the phones). Sheets follow the finger (`AnchoredDraggable`); the highlight cross-fades; transitions use `graphicsLayer` (a GPU layer, like Telegram's hardware layers) so nothing re-lays out while moving. |
| Shared, pre-built objects | Text styles, `Paint`/brushes and `TextMeasurer`s are created once in the theme and reused, so drawing allocates nothing (no GC pauses while scrolling). |
| Measured, not guessed | Macrobenchmark + baseline profile from M2; JankStats in debug builds; Perfetto traces for the synthesis threads. Every "it feels slow" goes through a trace first. |

---

## 13. Testing and profiling

| Level | What | Where it runs |
|---|---|---|
| Rust core | Unchanged: domain/app/infra tests in the main repo | laptop |
| Rust ffi | Record mappers, error mapping, `assemble` wiring with fakes; a host build smoke test (import an invented PDF, speak one sentence) | laptop |
| Kotlin pure | `core:model` helpers (toc tree, current chapter, labels, time left), contrast test, error registry mirror test | JVM, laptop |
| Kotlin + real core | `core:data` repositories against the **host** `liblectrice_ffi.so` (x86_64 Linux) through JNA: import, get document, speak one sentence, error names | JVM, laptop (needs the model; skips if missing) |
| ViewModels | With fakes of the `core:model` interfaces; `kotlinx-coroutines-test` | JVM |
| UI | A few Compose UI tests (open, play, jump, chapters sheet) on the phone | device |
| Playback | Manual checklist: screen off for a whole chapter, earbuds buttons, unplug headphones, a phone call (audio focus), process kill + resume from media controls | device |
| Performance | **Macrobenchmark** module: cold start, open a 400-page book, scroll the reader (`FrameTimingMetric`), open the chapters sheet. **Baseline profile** module (`androidx.baselineprofile` plugin) generated on the S26 Ultra (generation on a non-rooted phone needs Android 13+, UNVERIFIED) and shipped in the release build. JankStats in debug. | device |

- Test data follows the public-repo rule: **no text from the user's books.** The 400-page benchmark book is generated
  (invented text, built as a `document.json` loaded through a debug-only path). Real-book tests read `~/Desktop/*.pdf`
  pushed to the phone and are skipped when the files are missing.
- Targets for M2 (proposed): cold start < 500 ms to the library on the S26 Ultra and < 1 s on the S10+; reader scroll with
  < 1 % janky frames; opening a 400-page book < 1 s after the first time.

---

## 14. Licenses

### 14.1 What's inside the APK

| Part | License | Notes |
|---|---|---|
| Our Kotlin + Rust code | MIT OR Apache-2.0 | both repos (the main repo's README already notes that binaries built with sherpa-onnx 1.13.x include GPL-3 espeak-ng) |
| Kokoro 82M model + voices | Apache-2.0 | downloaded, not in the APK; NOTICE in the bundle |
| onnxruntime | MIT | in sherpa's libs |
| sherpa-onnx 1.13.8 | Apache-2.0, **but it statically includes espeak-ng (GPL-3.0)** | `strings` shows espeak inside `libsherpa-onnx-c-api.so` (report §3.5) |
| espeak-ng-data (19 MB in the model bundle) | GPL-3.0 | only needed with sherpa 1.x |
| sherpa-onnx **2.0.0** (released today, also on crates.io as 2.0.0) | Apache-2.0, **GPL-free** | PR #4055 "Remove espeak-ng and piper-phonemize from sherpa-onnx" (merged 2026-10-20). Kokoro's `data_dir` is now ignored. |
| pdfium (+ freetype, icu, libjpeg-turbo, …) | BSD-3 and others | show its `licenses/*.txt` |
| Jetpack, Kotlin, Media3, WorkManager | Apache-2.0 | |
| JNA | Apache-2.0 or LGPL-2.1 (dual) | we use it under Apache-2.0 |
| uniffi runtime | MPL-2.0 (UNVERIFIED in this pass) | file-level copyleft; we don't modify its files |
| Kokoro lexicon files | UNVERIFIED | check the source and license of `lexicon-us-en.txt` before publishing |

The "Open-source licenses" screen is a hand-kept list plus the license texts in `assets/licenses/`.

### 14.2 What sherpa-onnx 2.0.0 changes, and its catch

- **The good part:** with 2.0 nothing GPL is linked, and `espeak-ng-data` isn't needed. The APK and the model download can
  then be plain Apache/MIT/BSD.
- **The catch (verified in the v2.0.0 source, `offline-tts-kokoro-impl.h`):** without espeak, Kokoro's English front end is
  **lexicon-only**. A word that isn't in the lexicon is **skipped silently** ("OOV word skipped"); hyphenated words are split
  and each part looked up. Technical books are full of words a dictionary may lack (product names, `kubectl`, acronyms), so 2.0
  could drop words that 1.13.8 reads. (That 1.13.8 falls back to espeak for such words is likely, since it requires
  `data_dir`, but UNVERIFIED in its source.)
- **Decision (user 2026-10-23): try 2.0, and prefer it if the skipped-word rate is acceptable.** The spike compares 1.13.8
  and 2.0.0 on the phones (speed) and on the laptop (skipped words over the user's five books, counts only, no text in any
  file).
- **First, simple fixes in the core** (they run before the text reaches the engine, so they help any engine): spell all-caps
  acronyms letter by letter ("AWS" → "A W S"), split camelCase and letter-digit mixes ("getUserId" → "get user id",
  "k8s" → "k 8 s"), and add our own small extra lexicon file for common tech words that turn out to be missing.
- **"Acceptable"** is counted **after** those fixes, as skipped words ÷ spoken words (code blocks are already not spoken):
  - **on average over the five books ≤ 0.1 %**. At about 189 words per minute that is at most about one skipped word every
    5 minutes of listening;
  - **and in the worst book ≤ 0.25 %** (about one every 2 minutes);
  - **and the user's ear test passes:** 3 invented sentences full of tricky words, played on both versions; the user must
    not prefer 1.13.8 clearly.
- **Acceptable → move both apps to 2.0** and drop `espeak-ng-data`. That's a core release (C6).
- **Not acceptable →** stay on 1.13.8 and accept GPL-3 for the APK (next section). Look for a permissive grapheme-to-phoneme
  model for unknown words later (research needed; UNVERIFIED that a good one exists).
- The **desktop build has the same issue today**: the `lectrice-api` binary statically links sherpa 1.13.8, so a distributed
  desktop binary includes GPL-3 code too.

### 14.3 What it means for the public repos and for distribution (not legal advice)

**Distribution (user 2026-10-23): GitHub Releases only, no Play Store for now.** A signed release APK is built by GitHub
Actions (from M1/M2 on): the signing keystore is made once on the laptop with `keytool`, kept in a backup (losing it means
users can't update over the old install), and stored as Actions secrets (base64 keystore + passwords). Users update by
installing the newer APK; an in-app "new version available" check against the GitHub Releases API is possible later
(proposed, not v1).


- **Public source repos:** fine either way. The source is ours (Apache-2.0) and fetches GPL parts at build time; it doesn't
  commit them.
- **Distributing an APK built on sherpa 1.13.8** (on GitHub Releases): the APK as a whole must follow GPL-3.
  That means shipping the license texts, offering the complete corresponding source (our public repos at the release tag, plus
  the exact sherpa/espeak versions and build scripts), and adding no extra restrictions. Our Apache-2.0 code is compatible
  with that. GPL-3 also asks for "installation information" on consumer devices; Android lets users install their own build
  (sideloading), which is generally taken to be enough (UNVERIFIED legal detail, confirm before publishing).
- **On sherpa 2.0:** no GPL obligations; only notices.
- If Play is ever wanted: an AAB upload, 16 KB page alignment (all our `.so` files are already `0x4000` aligned),
  declarations for the foreground service types, and a Data safety form. None of it changes the architecture.

---

## 15. GPU and NPU acceleration (researched 2026-10-23)

The user asked: can the phone's GPU or NPU make speech faster and cheaper on battery?

### 15.1 Short answer

- **For v1: no, not in a way we can count on.** The realistic path is the **CPU**, through sherpa-onnx and ONNX Runtime (ORT).
  The spike still tries every accelerator that is a one-line change, and records honest numbers.
- **XNNPACK** (a faster CPU kernel library, not a GPU) is a one-line try. It might gain or lose a little.
- **NNAPI** (Android's old route to GPU/NPU) is a one-line try that will most likely do nothing for Kokoro. It is also
  deprecated.
- **The only real upside is the Snapdragon NPU (Qualcomm QNN / Hexagon)** on the S26 Ultra. But it needs model surgery and
  our own ORT session; sherpa-onnx doesn't offer it for TTS. That makes it a research project for after M1, not a setting.
- **GPU: no ready path today.**
- **Exynos S10+: no usable accelerator path.**

### 15.2 What sherpa-onnx exposes (verified in the v2.0.0 source)

- **Provider strings** (`sherpa-onnx/csrc/provider.cc`): `cpu`, `cuda`, `coreml`, `xnnpack`, `nnapi`, `trt`, `directml`,
  `spacemit`, `openvino`. Anything else falls back to `cpu` with a log line. On Android only **`cpu`, `xnnpack` and `nnapi`**
  make sense.
- **Fallback is safe:** if an execution provider (EP) isn't in the ORT build, or registering it fails, sherpa logs it and uses
  the CPU (`session.cc`: `xnnpack` checks `GetAvailableProviders`; `nnapi` falls back when the append fails).
- **Which ORT:** sherpa 2.0's Android build uses ORT **1.28.2** (`build-android-arm64-v8a.sh`). That is Microsoft's official
  `onnxruntime-android` package from Maven Central, repacked (`csukuangfj/onnxruntime-build`, `android.yaml`). Per the ORT
  docs, that package includes the NNAPI and XNNPACK EPs. The newest `onnxruntime-android` on Maven is 1.31.0 (2026-10-09).
- **The Rust crate already passes the provider:** `OfflineTtsModelConfig.provider: Option<String>` (sherpa-onnx 1.13.8
  `src/tts.rs`), so core change C6 only plumbs a setting.
- **One weakness:** sherpa registers XNNPACK without its thread-count option and without turning off ORT's thread spinning.
  The ORT docs recommend both, because otherwise two thread pools compete for the same cores. So the spike may undersell
  XNNPACK. Fixing it would mean a small sherpa patch (UNVERIFIED gain).
- **sherpa's own QNN, RKNN, Ascend and Axera backends are for speech *recognition* models only** (`sherpa-onnx/csrc/qnn/`:
  Paraformer, Whisper, SenseVoice, Zipformer, Moonshine, Parakeet, NeMo). They come in separate builds (`apk-qnn-*`
  workflows), not in the normal Android release. **None of them covers TTS.**

### 15.3 Does Kokoro's graph fit an accelerator? (verified on our `model.onnx`)

This was checked on the laptop with a small protobuf reader (no downloads) over `models/kokoro-multi-lang-v1_0/model.onnx`,
the fp32 model at opset 14.

- **Shapes are dynamic both ways.** Input `tokens` is `[1, sequence_length]`, `style` `[1, 256]`, `speed` `[1]`. The output
  `audio` is `[audio_length]`, and that length depends on the durations the model predicts. So the shapes **can't simply be
  fixed**; the model would have to be split.
- **The heavy math:** 102 `MatMul`, 90 `Conv`, 73 `Gemm`, 7 `ConvTranspose`, 70 `InstanceNormalization`, 6 `LSTM`.
- **Awkward ops for accelerators:** 1 `Loop`, 2 `If`, sequence ops (`SplitToSequence`, `SequenceAt`, `SequenceInsert`,
  `SequenceEmpty`, `ConcatFromSequence`), 7 `ScatterND`, `CumSum`, 4 `TopK`, 25 `Range`, `RandomNormalLike` /
  `RandomUniformLike`, and 51 `Sin` plus `Cos` and `Atan` (the vocoder), with hundreds of shape ops (`Shape` ×210, `Gather`,
  `Slice`, `Reshape`).

How each option covers it:

| Option | Covers | Kokoro fit | Expectation |
|---|---|---|---|
| **XNNPACK EP** (CPU kernels; in the ORT Android package) | `Conv`, `ConvTranspose`, `Gemm`, `MatMul`, `Resize`, `Softmax`, pools, `QLinear*` (ORT docs list) | The big fp32 convs and matmuls; everything else stays on the default CPU EP, so the graph is split many times with copies between the parts. How the int8 model's quantized ops map is UNVERIFIED. | Anywhere from a bit slower to a bit faster (UNVERIFIED); try it |
| **NNAPI EP** (Android ≥ 8.1; GPU/DSP/NPU through vendor drivers) | ORT docs list: no `LSTM`, `InstanceNormalization`, `Loop`, `If`, sequence ops, `ScatterND`, `CumSum`, `TopK`, `Range`, `Where`, `Expand`, `Equal`, `Cos`, `Atan` or random ops. And "NNAPI does not support dynamic input shapes" (ORT docs, *Make dynamic input shapes fixed*). | Takes nothing, or small islands | No gain, possibly slower, possibly driver bugs. Google deprecated NNAPI in Android 15 and points apps to LiteRT (UNVERIFIED wording: developer.android.com refused our requests today). No new work on it. |
| **QNN EP** (Snapdragon only; Maven `onnxruntime-android-qnn` 1.29.0, 2026-08-12) | HTP backend = the Hexagon NPU; also a `gpu` backend. HTP needs **static shapes** ("QNN EP does not support models with dynamic shapes") and is built around quantized models (uint8/uint16 QDQ, with calibration data). Newer HTPs can run fp32 graphs at fp16 precision (`enable_htp_fp16_precision`, default on). The ORT op list has no `LSTM`, `Loop`, `If`, sequence ops, `ScatterND`/`ScatterElements`, `CumSum`, `Range`, `GatherElements` or random ops. | Only after **model surgery**: (a) text encoder + duration predictor (LSTMs, dynamic) stay on the CPU; (b) the decoder/vocoder (convs) is exported at fixed lengths (e.g. 256/512/1024 frames, padded, then trimmed) for the NPU. Not available through sherpa: needs our own ORT session (Rust `ort` crate or ORT's Kotlin API) and our own Kokoro text front end. | The real upside on the **S26 Ultra**. ORT documents testing on SC8280, SM8350 and Snapdragon X; support for the S26 Ultra's chip and its `htp_arch` is UNVERIFIED. Snapdragon 855 (US S10+, Hexagon 690) is older than anything ORT tests: treat it as no. |
| **Exynos 9820 NPU** (EU/Korean S10+) | Only through NNAPI (Samsung's driver). Samsung's own SDK isn't used by ORT or sherpa, and its availability to apps is UNVERIFIED. | NNAPI doesn't fit (above) | **No usable path** |
| **GPU** | ORT has **no OpenCL EP** (no `providers/opencl` in ORT `main`, checked in the repo tree). The new **WebGPU EP** runs on Android over Vulkan (ORT docs), but ships as a separate "plugin EP" library; a prebuilt Android build and its Kokoro op coverage are UNVERIFIED, and sherpa doesn't expose it. QNN's `gpu` backend: op coverage UNVERIFIED. | Unknown | **No ready path.** For reference, Speech Central runs Kokoro on an Adreno GPU through OpenCL with its own engine (not ORT/sherpa) at RTF ≈ 0.93 on a 2020 mid-range phone (report §3.4). So a GPU can work, but there is no library we can drop in. |

### 15.4 Battery and heat

- On the CPU, synthesis keeps the big cores busy. The spike measures how warm the phone gets and whether it throttles.
- **Energy per minute of audio ≈ power × RTF.** A config that is twice as fast at the same power uses half the battery per
  minute of speech. So on the CPU, **RTF is the main battery lever.**
- NPUs usually do the same math for a fraction of the energy of the CPU, and GPUs land in between. Both are general claims,
  UNVERIFIED for Kokoro. That is why QNN is worth a look later.
- Levers we have in v1 without accelerators:
  - generate in bursts and then let the CPU sleep (read-ahead fills 10 minutes, then idles);
  - stop read-ahead while paused;
  - use the thread count that gives the best RTF per watt, which may not be the fastest one (the spike records both);
  - use int8 where it is faster.

### 15.5 In the spike (details: plan §5.2 "Accelerators" and §6)

- **Runs:** `cpu`, `xnnpack` and `nnapi`, on both phones and both sherpa versions, with fp32 and int8, at that phone's best
  thread count.
- **Recorded per run:** RTF, **battery mAh per minute of audio** (from `BatteryManager` charge counter, unplugged),
  temperature, and whether the provider really engaged (sherpa's log line "Use nnapi" vs "Fallback to cpu").
- **Correctness:** Kokoro's graph has random-noise ops, so the samples differ from run to run even on the CPU. Bytes can't be
  compared. The check is that the clip duration is within ±3 % of the CPU run, plus a short listen by the user.
- **Laptop desk check:** ORT's `check_onnx_model_mobile_usability` on `model.onnx`, which reports how much of the graph NNAPI
  could take. It needs `pip install onnxruntime` (download later, plan §3a).
- **Honest expectation:** XNNPACK about the same as the CPU; NNAPI no gain, or a fallback; QNN and GPU not testable in M1.

### 15.6 After M1, only if wanted

A **QNN prototype on the S26 Ultra**: split the model, export fixed-length buckets, run them through the QNN EP, and wrap it
all as a second `SpeechEngine` adapter ("Kokoro (NPU)"). The engine trait keeps this clean, and its own engine id keeps its
cache separate. A rough estimate is 2–3 weeks of research (UNVERIFIED). Worth it only if the CPU result on the S26 Ultra is
amber/red, or the battery drain per hour of listening is bad.

---

## 16. Milestones (from the report §4.4, updated)

| # | Milestone | Done when |
|---|---|---|
| **M0** | Core release `v0.2.0` (C1–C9) in the main repo (the `lectrice-android` repo already exists) | Desktop tests pass with the features on; the ffi crate builds for the host and for `aarch64-linux-android` |
| **M1** | **Go/no-go spike** (`docs/2026-10-23-android-m1-spike-plan.md`) | The measurements are written down and the user has decided |
| M2 | Library + reader + chapters sheet + design system, playback faked | Smooth scroll on the 400-page benchmark book, light and dark, no main-thread core calls |
| M3 | Background playback (Media3, DataSource, read-ahead, lock screen, Bluetooth) | A whole chapter with the screen off, controlled from earbuds |
| M4 | Model download (both bundles), resumable M4B export, Prepare mode (only if needed), licenses, signed APK on GitHub Releases via Actions, polish, baseline profile | An 8-hour book exports across the 6-hour limit; first-run download resumes after airplane mode |

---

## 17. Risks (short; details in the report §5)

1. **Kokoro speed on the phones** (the spike decides). Fallbacks are on the phone only: Prepare while charging, int8, the best
   thread count, then a lighter engine adapter.
2. **Memory** (about 830 MB reported on iOS) → lazy load, unload when idle, measured in the spike.
3. **sherpa 2.0 skips unknown words** (§14.2) → the OOV count task.
4. **Native loading** (pdfium by bare name; the sherpa `.so` chain) → spike.
5. **6-hour limit and Samsung battery killers** → parts + pause/resume.
6. **M4B chapters with MediaMuxer** → `chpl` post-step, test in M4.
7. **Two repos drifting** → §3.4, pinned tag, one `assemble`.
8. **Version churn** (sherpa 2.0 today, uniffi 0.32, Media3 1.11): pin everything in `libs.versions.toml` and `Cargo.lock`.
   The AGP and Compose BOM versions are UNVERIFIED (Google's Maven wasn't reachable from here); the spike pins them.

## 18. Answered questions and what's still open

Answered by the user (2026-10-23):

- Model: **the user chooses** fp32 or int8 at first run and can switch later (§6.1).
- sherpa-onnx 2.0: **try it; prefer it if the skipped-word rate is acceptable** (§14.2 defines "acceptable").
- Prepare while charging: **only if the spike shows it's needed** (§8.5).
- Distribution: **GitHub Releases only** (§14.3).
- Lock screen / Bluetooth "next": **next sentence** (§7.1).
- Repo: `kia-salvati/lectrice-android`, public (§3).

Still open (after the spike):

1. If 2.0 fails the skipped-word test: accept GPL-3 for the APK, or wait for a permissive fix?
2. Is the accelerator follow-up (§15.6: QNN on the S26 Ultra) worth a few weeks of model work if the CPU is already fast enough?
