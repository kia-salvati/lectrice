//! Fakes for the export tests: an exporter that records what it is given, a record store and a decoder that
//! reads the speech fakes' `CountingEncoder` output.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use lectrice_domain::{
    AppError, AppResult, Audio, AudioDecoder, AudioExporter, ChapterMark, ErrorName, ExportId, ExportPart, ExportRecord, ExportRecordStore, ExportTarget,
    ExportWriter, ExportedFile,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Event {
    BeginPart { index: u32, sample_rate: u32 },
    Append(usize),
    FinishPart(u32),
    Abandoned(u32),
    Assemble { title: String, artist: String, parts: Vec<u32>, chapters: Vec<(String, u64)> },
    Discard,
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
        self.events().iter().filter(|event| matches!(event, Event::BeginPart { .. })).count()
    }

    pub fn clear(&self) {
        self.events.lock().unwrap().clear();
    }
}

impl AudioExporter for FakeExporter {
    fn check_available(&self) -> AppResult<()> {
        if self.unavailable.load(Ordering::SeqCst) {
            return Err(AppError::new(ErrorName::ExporterUnavailable));
        }
        Ok(())
    }

    fn begin_part(&self, target: &ExportTarget, part_index: u32, sample_rate: u32) -> AppResult<Box<dyn ExportWriter>> {
        self.check_available()?;
        self.events.lock().unwrap().push(Event::BeginPart { index: part_index, sample_rate });
        let path = PathBuf::from(format!("/exports/parts/{}/{part_index}.wav", target.export_id));
        let fail_appends = self.fail_appends.load(Ordering::SeqCst);
        Ok(Box::new(FakeWriter { events: self.events.clone(), fail_appends, index: part_index, path, sample_rate, samples: 0, finished: false }))
    }

    fn assemble(&self, target: &ExportTarget, parts: &[ExportPart], chapters: &[ChapterMark]) -> AppResult<ExportedFile> {
        self.events.lock().unwrap().push(Event::Assemble {
            title: target.title.clone(),
            artist: target.artist.clone(),
            parts: parts.iter().map(|part| part.index).collect(),
            chapters: chapters.iter().map(|chapter| (chapter.title.clone(), chapter.start_ms)).collect(),
        });
        let path = PathBuf::from(format!("/exports/{}/{}.{}", target.document_id, target.voice_id, target.format.extension()));
        Ok(ExportedFile { path, size_bytes: 1 })
    }

    fn discard_parts(&self, _export_id: &ExportId) -> AppResult<()> {
        self.events.lock().unwrap().push(Event::Discard);
        Ok(())
    }
}

struct FakeWriter {
    events: Arc<Mutex<Vec<Event>>>,
    fail_appends: bool,
    index: u32,
    path: PathBuf,
    sample_rate: u32,
    samples: u64,
    finished: bool,
}

impl ExportWriter for FakeWriter {
    fn append(&mut self, audio: &Audio) -> AppResult<()> {
        if self.fail_appends {
            return Err(AppError::new(ErrorName::Internal).with_detail("fake encoder failure"));
        }
        self.samples += audio.samples.len() as u64;
        self.events.lock().unwrap().push(Event::Append(audio.samples.len()));
        Ok(())
    }

    fn finish(mut self: Box<Self>) -> AppResult<ExportPart> {
        self.finished = true;
        self.events.lock().unwrap().push(Event::FinishPart(self.index));
        Ok(ExportPart { index: self.index, path: self.path.clone(), samples: self.samples, sample_rate: self.sample_rate })
    }
}

impl Drop for FakeWriter {
    fn drop(&mut self) {
        if !self.finished {
            self.events.lock().unwrap().push(Event::Abandoned(self.index));
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
