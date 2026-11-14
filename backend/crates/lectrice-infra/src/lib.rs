//! Adapters that implement the domain ports with real libraries, files and programs.
//!
//! Features (all on by default, for the desktop): `pdfium` (PDF text), `kokoro` (the voice, sherpa-onnx),
//! `sherpa-static` (links sherpa-onnx statically; off on Android, which ships its shared libraries) and
//! `ffmpeg-export` (exports through the system ffmpeg; Android brings its own exporter) and `time-stretch` (exports
//! with a baked-in speed). Off by default: `helper-client` (the phone's side of the computer helper, file 24).

#[cfg(feature = "time-stretch")]
pub mod audio;
pub mod clock;
pub mod export;
#[cfg(feature = "helper-client")]
pub mod helper;
#[cfg(feature = "pdfium")]
pub mod pdf;
pub mod speech;
pub mod storage;
mod sync;
pub mod text;
mod units;

#[cfg(feature = "time-stretch")]
pub use audio::SonicTimeStretch;
pub use clock::SystemClock;
#[cfg(feature = "ffmpeg-export")]
pub use export::FfmpegExporter;
#[cfg(feature = "pdfium")]
pub use pdf::{PdfLimits, PdfiumTextSource};
pub use speech::{DeferredEngineInfo, DeferredSpeechEngine, WavDecoder, WavEncoder};
#[cfg(feature = "kokoro")]
pub use speech::{KokoroSettings, KokoroSpeechEngine, KokoroVariant};
pub use storage::{CachedLibrary, FsAudioCache, FsDocumentRepository, FsExportRecordStore, FsVoiceSampleCache};
pub use text::{SentencexSplitter, Sha256ContentHasher};
