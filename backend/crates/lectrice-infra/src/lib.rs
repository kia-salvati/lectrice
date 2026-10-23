//! Adapters that implement the domain ports with real libraries, files and programs.

pub mod clock;
pub mod export;
pub mod pdf;
pub mod speech;
pub mod storage;
pub mod text;

pub use clock::SystemClock;
pub use export::FfmpegExporter;
pub use pdf::PdfiumTextSource;
pub use speech::{KokoroSettings, KokoroSpeechEngine, WavDecoder, WavEncoder};
pub use storage::{CachedLibrary, FsAudioCache, FsDocumentRepository, FsExportRecordStore};
pub use text::{SentencexSplitter, Sha256ContentHasher};
