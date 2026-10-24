//! Adapters that implement the domain ports with real libraries, files and programs.
//!
//! Features (all on by default, for the desktop): `pdfium` (PDF text), `kokoro` (the voice, sherpa-onnx),
//! `sherpa-static` (links sherpa-onnx statically; off on Android, which ships its shared libraries) and
//! `ffmpeg-export` (exports through the system ffmpeg; Android brings its own exporter) and `time-stretch` (exports
//! with a baked-in speed).

#[cfg(feature = "time-stretch")]
pub mod audio;
pub mod clock;
#[cfg(feature = "ffmpeg-export")]
pub mod export;
#[cfg(feature = "pdfium")]
pub mod pdf;
pub mod speech;
pub mod storage;
pub mod text;

#[cfg(feature = "time-stretch")]
pub use audio::SonicTimeStretch;
pub use clock::SystemClock;
#[cfg(feature = "ffmpeg-export")]
pub use export::FfmpegExporter;
#[cfg(feature = "pdfium")]
pub use pdf::PdfiumTextSource;
#[cfg(feature = "kokoro")]
pub use speech::{KokoroSettings, KokoroSpeechEngine, KokoroVariant};
pub use speech::{WavDecoder, WavEncoder};
pub use storage::{CachedLibrary, FsAudioCache, FsDocumentRepository, FsExportRecordStore, FsVoiceSampleCache};
pub use text::{SentencexSplitter, Sha256ContentHasher};
