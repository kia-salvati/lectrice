# Changelog

All notable changes to Lectrice are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html). The version in `backend/Cargo.toml`
(`[workspace.package]`) equals the release tag.

"Core" is the shared Rust library (`lectrice-domain`, `lectrice-app`, `lectrice-infra`) that the Android app
pulls by tag. From 1.1.0 on, a change to a core trait, public type or saved format raises the major version.
(1.1.0 still breaks some 1.0 traits: 1.0 had no users outside this repo.)

## [Unreleased]

## [1.1.0] - 2026-10-23

### Core

#### Added
- A table of contents per document (`toc`: title, level 1–3, first sentence, page), from the PDF outline when it
  matches the headings, otherwise from the detected headings. Cleaner version 3, so saved books are cleaned again.
- `lectrice_app::services`: `Adapters`, `Settings`, `Services`, `assemble()` and the `DocumentServices`,
  `SpeechServices` and `ExportServices` structs, moved from `lectrice-api` so every shell wires the core the same way.
- Resumable exports. An export is written as numbered parts of about 10 listening-minutes and the finished parts are
  recorded in its export record. New `paused` state; `PauseAudioExport` keeps the finished parts,
  `ResumeAudioExports` queues paused exports again, and starting the same export again continues it.
  `CancelAudioExport` deletes the parts.
- `AudioExport` gains `partsDone` and `partsTotal`; `ExportRecord` gains `parts` and `chapters`.
- `lectrice_app::speech::playback_order(document, read_skipped) -> Vec<SentenceId>`: the one playback order for
  the player, the read-ahead and exports.
- `lectrice-infra` features: `pdfium`, `kokoro`, `sherpa-static` and `ffmpeg-export` (all on by default).
- `KokoroSettings::provider` (the onnxruntime execution provider: `cpu`, or `xnnpack` / `nnapi` on Android).
- Error names for both apps: `ModelMissing` (4001), `ModelDownloadFailed` (4002), `ModelChecksumMismatch` (4003),
  `StorageFull` (4004) and `ExportPaused` (4005).

#### Changed
- **Breaking:** `AudioExporter` now has `begin_part`, `assemble` and `discard_parts` instead of `begin`, and
  `ExportWriter::finish` returns an `ExportPart`; chapter marks go to `assemble` instead of `start_chapter`.
  `ExportTarget` gains `export_id`.
- **Breaking:** `export_plan` takes `part_minutes`; `ExportSettings` gains `part_minutes` (default 10).
- **Breaking:** the old `playback_order` (with block indexes) is now crate-private as `playable_in_order`.
- `FfmpegExporter` writes 16-bit mono WAV parts and joins them with one ffmpeg concat (MP3, or M4B with chapters).
- An export a stop interrupted is now `paused` (it was `failed`); the desktop server resumes it at start.
- `ExporterUnavailable` now reads "Audio export isn't available."; the ffmpeg detail stays in the log.
- The workspace `sherpa-onnx` dependency no longer turns on `static`; `lectrice-infra`'s `sherpa-static` does.
- `PdfiumTextSource::new` documents that a bare library name lets the system linker find pdfium.

#### Migration
- Export records saved by 1.0 load as they are (the new fields default to empty).
- An `AudioExporter` adapter implements `begin_part`, `assemble` and `discard_parts`; see `FfmpegExporter`.
- A shell builds `lectrice_app::services::Adapters` and `Settings` and calls `assemble`, then calls
  `exports.resume` when it is ready for paused exports to continue.
- `Settings::upload_max_bytes` is now `import_max_bytes`.

### Desktop server
- New `[speech] provider` config key (default `"cpu"`).

### Web app
- Chapters: a closable whole-book outline rail on wide screens, a bottom sheet elsewhere, and
  "Ch. 7 · Title · p. 213" in the player pill.

## [1.0.0] - 2026-10-23

The first release: PDF → cleaned reading view → Kokoro `af_heart` reading sentence by sentence, with read-ahead,
an audio cache, reading positions, recent documents and MP3/M4B export. (The workspace version stayed at 0.1.0
in this release.)

[Unreleased]: https://github.com/kia-salvati/lectrice/compare/v1.1.0...HEAD
[1.1.0]: https://github.com/kia-salvati/lectrice/compare/v1.0.0...v1.1.0
[1.0.0]: https://github.com/kia-salvati/lectrice/releases/tag/v1.0.0
