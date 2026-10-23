//! The app wired for route tests: real storage, cache and WAV encoder in a temp dir, fake PDF reading,
//! cleaning, splitting and voice, so no model or pdfium is needed.

use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};

use axum::Router;
use axum::body::{Body, Bytes};
use axum::http::{HeaderMap, HeaderValue, Request, StatusCode, header};
use http_body_util::BodyExt;
use lectrice_app::exports::ExportSettings;
use lectrice_domain::{
    AppError, AppResult, Audio, AudioExporter, BlockKind, CleanedBlock, CleanedDocument, DocumentCleaner, ErrorName, ExportTarget, ExportWriter, ExportedFile,
    PdfContent, PdfTextSource, SentenceSplitter, SpeechEngine, SpeechRequest, TextSpan, Voice, VoiceGender, VoiceId, VoiceLanguage,
};
use lectrice_infra::{FsAudioCache, FsDocumentRepository, FsExportRecordStore, Sha256ContentHasher, SystemClock, WavDecoder, WavEncoder};
use serde_json::Value;
use tempfile::TempDir;
use tower::ServiceExt;

use crate::app::build_app;
use crate::composition::{Adapters, Settings, assemble};
use lectrice_app::speech::ReadAheadSettings;

pub const UPLOAD_MAX_BYTES: usize = 4096;
pub const PDF_BYTES: &[u8] = b"%PDF-1.7 a small fake book";
/// The fake PDF reader finds no text in a PDF that contains this marker.
pub const NO_TEXT_PDF_BYTES: &[u8] = b"%PDF-1.7 NOTEXT scanned pages";
pub const DEFAULT_VOICE: &str = "af_heart";
const TEST_ORIGIN: &str = "http://localhost:5173";
const BOUNDARY: &str = "lectrice-test-boundary";
const NO_TEXT_MARKER: &[u8] = b"NOTEXT";
const FAKE_SAMPLE_RATE: u32 = 24_000;
const FAKE_SAMPLES_PER_CHAR: usize = 240;
const AUDIO_CACHE_MAX_BYTES: u64 = 64 * 1024 * 1024;
const NO_READ_AHEAD: ReadAheadSettings = ReadAheadSettings { read_ahead_minutes: 0, keep_minutes_per_document: 60 };
/// The spec defaults (§11).
pub const SPEC_READ_AHEAD: ReadAheadSettings = ReadAheadSettings { read_ahead_minutes: 10, keep_minutes_per_document: 60 };

/// Sentence ids of the fake book (see [`FakeCleaner`]).
pub mod sentences {
    pub const HEADING: u32 = 0;
    pub const FIRST: u32 = 1;
    pub const ONLY_URL: u32 = 3;
    pub const CODE_CUE: u32 = 5;
    pub const COUNT: u32 = 6;
}

struct FakePdfSource;

impl PdfTextSource for FakePdfSource {
    fn read_runs(&self, pdf: &[u8]) -> AppResult<PdfContent> {
        if pdf.windows(NO_TEXT_MARKER.len()).any(|window| window == NO_TEXT_MARKER) {
            return Err(AppError::new(ErrorName::PdfNoText));
        }
        Ok(PdfContent::default())
    }
}

/// A heading, a two-sentence paragraph, a URL-only paragraph (speech text "") and a code block with its cue.
struct FakeCleaner;

fn span(text: &str, spoken: bool) -> TextSpan {
    TextSpan { text: text.to_string(), page_index: 1, spoken }
}

fn block(kind: BlockKind, spans: Vec<TextSpan>) -> CleanedBlock {
    CleanedBlock { kind, level: None, page_index: 1, spans }
}

impl DocumentCleaner for FakeCleaner {
    fn version(&self) -> u32 {
        1
    }

    fn clean(&self, _content: PdfContent) -> AppResult<CleanedDocument> {
        Ok(CleanedDocument {
            title: Some("Fake Book".into()),
            page_count: 2,
            pages: Vec::new(),
            blocks: vec![
                block(BlockKind::Heading, vec![span("Intro", true)]),
                block(BlockKind::Paragraph, vec![span("One. Two.", true)]),
                block(BlockKind::Paragraph, vec![span("https://example.com", false)]),
                block(BlockKind::Code, vec![span("let x = 1;", true)]),
            ],
        })
    }
}

/// Ends a sentence at a `.` followed by whitespace or the end of the text.
struct FakeSplitter;

impl SentenceSplitter for FakeSplitter {
    fn split(&self, text: &str) -> Vec<Range<usize>> {
        let mut ranges = Vec::new();
        let mut start = 0;
        let bytes = text.as_bytes();
        for (index, byte) in bytes.iter().enumerate() {
            let at_end = bytes.get(index + 1).is_none_or(u8::is_ascii_whitespace);
            if *byte == b'.' && at_end {
                ranges.push(start..index + 1);
                start = index + 1;
            }
        }
        ranges.push(start..text.len());
        ranges.into_iter().filter_map(|range| trim(text, range)).collect()
    }
}

fn trim(text: &str, range: Range<usize>) -> Option<Range<usize>> {
    let piece = &text[range.clone()];
    let start = range.start + (piece.len() - piece.trim_start().len());
    let trimmed = piece.trim();
    (!trimmed.is_empty()).then(|| start..start + trimmed.len())
}

/// Speaks `af_heart` only; counts its calls.
#[derive(Default)]
pub struct FakeEngine {
    calls: AtomicUsize,
}

impl FakeEngine {
    pub fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl SpeechEngine for FakeEngine {
    fn id(&self) -> &str {
        "fake-engine"
    }

    fn voices(&self) -> Vec<Voice> {
        vec![Voice { id: VoiceId::new(DEFAULT_VOICE), name: "Heart".into(), language: VoiceLanguage::EnUs, gender: VoiceGender::Female }]
    }

    fn sample_rate(&self) -> u32 {
        FAKE_SAMPLE_RATE
    }

    fn synthesize(&self, request: SpeechRequest) -> AppResult<Audio> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(Audio { samples: vec![0.25; request.text.len() * FAKE_SAMPLES_PER_CHAR], sample_rate: FAKE_SAMPLE_RATE })
    }
}

/// Writes `"FAKE:<samples appended>"` to the real export path. It can be unavailable, or hold every export at its start.
pub struct TestExporter {
    data_dir: PathBuf,
    unavailable: bool,
    held: Mutex<bool>,
    changed: Condvar,
    written: Arc<Mutex<Vec<u8>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExporterMode {
    Ready,
    Unavailable,
    Held,
}

impl TestExporter {
    fn new(data_dir: &Path, unavailable: bool, held: bool) -> Self {
        Self { data_dir: data_dir.to_path_buf(), unavailable, held: Mutex::new(held), changed: Condvar::new(), written: Arc::default() }
    }

    /// The bytes of the last finished export.
    pub fn file_bytes(&self) -> Vec<u8> {
        self.written.lock().unwrap().clone()
    }

    pub fn release(&self) {
        *self.held.lock().unwrap() = false;
        self.changed.notify_all();
    }
}

impl AudioExporter for TestExporter {
    fn check_available(&self) -> AppResult<()> {
        if self.unavailable { Err(AppError::new(ErrorName::ExporterUnavailable)) } else { Ok(()) }
    }

    fn begin(&self, target: &ExportTarget, _sample_rate: u32) -> AppResult<Box<dyn ExportWriter>> {
        self.check_available()?;
        let mut held = self.held.lock().unwrap();
        while *held {
            held = self.changed.wait(held).unwrap();
        }
        let path = self.data_dir.join("exports").join(target.document_id.as_str()).join(format!("{}.{}", target.voice_id, target.format.extension()));
        Ok(Box::new(TestExportWriter { path, samples: 0, written: self.written.clone() }))
    }
}

struct TestExportWriter {
    path: PathBuf,
    samples: usize,
    written: Arc<Mutex<Vec<u8>>>,
}

impl ExportWriter for TestExportWriter {
    fn start_chapter(&mut self, _title: &str) -> AppResult<()> {
        Ok(())
    }

    fn append(&mut self, audio: &Audio) -> AppResult<()> {
        self.samples += audio.samples.len();
        Ok(())
    }

    fn finish(self: Box<Self>) -> AppResult<ExportedFile> {
        let bytes = format!("FAKE:{}", self.samples).into_bytes();
        std::fs::create_dir_all(self.path.parent().unwrap()).unwrap();
        std::fs::write(&self.path, &bytes).unwrap();
        *self.written.lock().unwrap() = bytes.clone();
        Ok(ExportedFile { path: self.path.clone(), size_bytes: bytes.len() as u64 })
    }
}

pub struct TestApp {
    pub router: Router,
    pub engine: Arc<FakeEngine>,
    pub exporter: Arc<TestExporter>,
    pub data_dir: TempDir,
}

impl Drop for TestApp {
    /// A held export would block the runner thread from stopping.
    fn drop(&mut self) {
        self.exporter.release();
    }
}

impl TestApp {
    /// Read-ahead is off, so the engine only speaks the requested sentences.
    pub fn new() -> Self {
        Self::with_read_ahead(NO_READ_AHEAD)
    }

    pub fn with_read_ahead(read_ahead: ReadAheadSettings) -> Self {
        let engine = Arc::new(FakeEngine::default());
        Self::build(Some(engine.clone()), engine, read_ahead, ExporterMode::Ready)
    }

    /// With a real engine; `engine` call counts then stay 0.
    pub fn with_engine(engine: Arc<dyn SpeechEngine>) -> Self {
        Self::build(Some(engine), Arc::default(), NO_READ_AHEAD, ExporterMode::Ready)
    }

    /// The app as it runs when the speech engine failed to load.
    pub fn without_engine() -> Self {
        Self::build(None, Arc::default(), NO_READ_AHEAD, ExporterMode::Ready)
    }

    /// As when ffmpeg is missing.
    pub fn without_exporter() -> Self {
        let engine = Arc::new(FakeEngine::default());
        Self::build(Some(engine.clone()), engine, NO_READ_AHEAD, ExporterMode::Unavailable)
    }

    /// Every export waits at its start until the app is dropped.
    pub fn with_blocked_exports() -> Self {
        let engine = Arc::new(FakeEngine::default());
        Self::build(Some(engine.clone()), engine, NO_READ_AHEAD, ExporterMode::Held)
    }

    fn build(engine: Option<Arc<dyn SpeechEngine>>, fake: Arc<FakeEngine>, read_ahead: ReadAheadSettings, mode: ExporterMode) -> Self {
        let data_dir = TempDir::new().unwrap();
        let exporter = Arc::new(TestExporter::new(data_dir.path(), mode == ExporterMode::Unavailable, mode == ExporterMode::Held));
        let router = router_with(data_dir.path(), engine, read_ahead, exporter.clone());
        Self { router, engine: fake, exporter, data_dir }
    }

    pub async fn send(&self, request: Request<Body>) -> (StatusCode, HeaderMap, Bytes) {
        let response = self.router.clone().oneshot(request).await.unwrap();
        let (parts, body) = response.into_parts();
        (parts.status, parts.headers, body.collect().await.unwrap().to_bytes())
    }

    pub async fn json(&self, request: Request<Body>) -> (StatusCode, Value) {
        let (status, _, bytes) = self.send(request).await;
        (status, serde_json::from_slice(&bytes).unwrap())
    }

    /// Uploads [`PDF_BYTES`] and returns the new document id.
    pub async fn import_book(&self) -> String {
        let (status, body) = self.json(upload("Book.pdf", PDF_BYTES)).await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
        body["document"]["id"].as_str().unwrap().to_string()
    }
}

fn router_with(data_dir: &Path, engine: Option<Arc<dyn SpeechEngine>>, read_ahead: ReadAheadSettings, exporter: Arc<TestExporter>) -> Router {
    let library = Arc::new(FsDocumentRepository::new(data_dir));
    let audio_cache = Arc::new(FsAudioCache::open(data_dir, AUDIO_CACHE_MAX_BYTES).unwrap());
    let adapters = Adapters {
        hasher: Arc::new(Sha256ContentHasher),
        pdf_source: Arc::new(FakePdfSource),
        cleaner: Arc::new(FakeCleaner),
        splitter: Arc::new(FakeSplitter),
        reader: library.clone(),
        writer: library,
        clock: Arc::new(SystemClock),
        speech_engine: engine,
        audio_cache: audio_cache.clone(),
        audio_retention: audio_cache,
        audio_encoder: Arc::new(WavEncoder),
        audio_decoder: Arc::new(WavDecoder),
        exporter,
        export_records: Arc::new(FsExportRecordStore::new(data_dir)),
    };
    let settings = Settings { upload_max_bytes: UPLOAD_MAX_BYTES, default_voice: VoiceId::new(DEFAULT_VOICE), read_ahead, export: ExportSettings::default() };
    let state = assemble(adapters, settings).unwrap();
    build_app(state, HeaderValue::from_static(TEST_ORIGIN))
}

pub fn get(path: &str) -> Request<Body> {
    Request::builder().uri(path).body(Body::empty()).unwrap()
}

pub fn put_json(path: &str, body: &str) -> Request<Body> {
    Request::builder().method("PUT").uri(path).header(header::CONTENT_TYPE, "application/json").body(Body::from(body.to_string())).unwrap()
}

/// A multipart upload with one `file` part.
pub fn upload(file_name: &str, bytes: &[u8]) -> Request<Body> {
    multipart(&[("file", Some(file_name), bytes)])
}

pub fn multipart(parts: &[(&str, Option<&str>, &[u8])]) -> Request<Body> {
    let mut body = Vec::new();
    for (name, file_name, bytes) in parts {
        let file_name = file_name.map(|file_name| format!("; filename=\"{file_name}\"")).unwrap_or_default();
        body.extend_from_slice(
            format!("--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"{name}\"{file_name}\r\nContent-Type: application/pdf\r\n\r\n").as_bytes(),
        );
        body.extend_from_slice(bytes);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{BOUNDARY}--\r\n").as_bytes());
    Request::builder()
        .method("POST")
        .uri(crate::routes::DOCUMENTS_PATH)
        .header(header::CONTENT_TYPE, format!("multipart/form-data; boundary={BOUNDARY}"))
        .body(Body::from(body))
        .unwrap()
}

pub fn error_name(body: &Value) -> &str {
    body["error"]["name"].as_str().unwrap_or_default()
}
