//! Fakes for the export tests: an exporter that records what it is given, a record store and a decoder that
//! reads the speech fakes' `CountingEncoder` output.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use lectrice_domain::{
    AppError, AppResult, Audio, AudioDecoder, AudioExporter, ErrorName, ExportId, ExportRecord, ExportRecordStore, ExportTarget, ExportWriter, ExportedFile,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Event {
    Begin { title: String, artist: String, sample_rate: u32 },
    Chapter(String),
    Append(usize),
    Finish,
    Abandoned,
}

#[derive(Default)]
pub(crate) struct FakeExporter {
    pub unavailable: AtomicBool,
    pub fail_appends: AtomicBool,
    events: Arc<Mutex<Vec<Event>>>,
}

impl FakeExporter {
    pub fn events(&self) -> Vec<Event> {
        self.events.lock().unwrap().clone()
    }

    pub fn begins(&self) -> usize {
        self.events().iter().filter(|event| matches!(event, Event::Begin { .. })).count()
    }
}

impl AudioExporter for FakeExporter {
    fn check_available(&self) -> AppResult<()> {
        if self.unavailable.load(Ordering::SeqCst) {
            return Err(AppError::new(ErrorName::ExporterUnavailable));
        }
        Ok(())
    }

    fn begin(&self, target: &ExportTarget, sample_rate: u32) -> AppResult<Box<dyn ExportWriter>> {
        self.check_available()?;
        self.events.lock().unwrap().push(Event::Begin { title: target.title.clone(), artist: target.artist.clone(), sample_rate });
        let path = PathBuf::from(format!("/exports/{}/{}.{}", target.document_id, target.voice_id, target.format.extension()));
        Ok(Box::new(FakeWriter { events: self.events.clone(), fail_appends: self.fail_appends.load(Ordering::SeqCst), path, finished: false }))
    }
}

struct FakeWriter {
    events: Arc<Mutex<Vec<Event>>>,
    fail_appends: bool,
    path: PathBuf,
    finished: bool,
}

impl ExportWriter for FakeWriter {
    fn start_chapter(&mut self, title: &str) -> AppResult<()> {
        self.events.lock().unwrap().push(Event::Chapter(title.to_string()));
        Ok(())
    }

    fn append(&mut self, audio: &Audio) -> AppResult<()> {
        if self.fail_appends {
            return Err(AppError::new(ErrorName::Internal).with_detail("fake encoder failure"));
        }
        self.events.lock().unwrap().push(Event::Append(audio.samples.len()));
        Ok(())
    }

    fn finish(mut self: Box<Self>) -> AppResult<ExportedFile> {
        self.finished = true;
        self.events.lock().unwrap().push(Event::Finish);
        Ok(ExportedFile { path: self.path.clone(), size_bytes: 1 })
    }
}

impl Drop for FakeWriter {
    fn drop(&mut self) {
        if !self.finished {
            self.events.lock().unwrap().push(Event::Abandoned);
        }
    }
}

#[derive(Default)]
pub(crate) struct MemoryRecordStore {
    records: Mutex<HashMap<ExportId, ExportRecord>>,
}

impl MemoryRecordStore {
    pub fn get(&self, id: &ExportId) -> Option<ExportRecord> {
        self.records.lock().unwrap().get(id).cloned()
    }
}

impl ExportRecordStore for MemoryRecordStore {
    fn load_all(&self) -> AppResult<Vec<ExportRecord>> {
        Ok(self.records.lock().unwrap().values().cloned().collect())
    }

    fn save(&self, record: &ExportRecord) -> AppResult<()> {
        self.records.lock().unwrap().insert(record.export.id.clone(), record.clone());
        Ok(())
    }

    fn delete(&self, id: &ExportId) -> AppResult<()> {
        self.records.lock().unwrap().remove(id);
        Ok(())
    }
}

/// Reads `"<sample rate>:<sample count>"` (what `CountingEncoder` writes) back as silence of that length.
pub(crate) struct CountingDecoder;

impl AudioDecoder for CountingDecoder {
    fn decode(&self, bytes: &[u8]) -> AppResult<Audio> {
        let text = String::from_utf8_lossy(bytes);
        let parsed = text.split_once(':').and_then(|(rate, count)| Some((rate.parse().ok()?, count.parse().ok()?)));
        let (sample_rate, count): (u32, usize) = parsed.ok_or_else(|| AppError::new(ErrorName::Internal).with_detail("not a counting clip"))?;
        Ok(Audio { samples: vec![0.0; count], sample_rate })
    }
}
