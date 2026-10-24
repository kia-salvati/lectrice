//! Fakes for the export tests: an exporter that records what it is given (and can run out of space), a record
//! store, a decoder that reads the speech fakes' `CountingEncoder` output and a time-stretch that only shortens.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use lectrice_domain::{
    AppError, AppResult, Audio, AudioDecoder, AudioExporter, ChapterMark, ErrorName, ExportId, ExportPart, ExportRecord, ExportRecordStore, ExportSpeed,
    ExportTarget, ExportWriter, ExportedFile, TimeStretch, TimeStretcher,
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
    /// Parts from this index on run out of space while appending.
    pub full_from_part: Mutex<Option<u32>>,
    pub full_when_assembling: AtomicBool,
    events: Arc<Mutex<Vec<Event>>>,
    targets: Mutex<Vec<ExportTarget>>,
    deleted: Mutex<Vec<PathBuf>>,
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

    /// The targets `assemble` was given, in order.
    pub fn assembled(&self) -> Vec<ExportTarget> {
        self.targets.lock().unwrap().clone()
    }

    pub fn deleted(&self) -> Vec<PathBuf> {
        self.deleted.lock().unwrap().clone()
    }

    pub fn chapters(&self) -> Vec<Vec<(String, u64)>> {
        self.events()
            .into_iter()
            .filter_map(|event| match event {
                Event::Assemble { chapters, .. } => Some(chapters),
                _ => None,
            })
            .collect()
    }
}

fn storage_full() -> AppError {
    AppError::new(ErrorName::StorageFull).and(ErrorName::StorageFailed).with_detail("fake disk full")
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
        let failure = if self.fail_appends.load(Ordering::SeqCst) {
            Some(Failure::Broken)
        } else if self.full_from_part.lock().unwrap().is_some_and(|from| part_index >= from) {
            Some(Failure::Full)
        } else {
            None
        };
        Ok(Box::new(FakeWriter { events: self.events.clone(), failure, index: part_index, path, sample_rate, samples: 0, finished: false }))
    }

    fn assemble(&self, target: &ExportTarget, parts: &[ExportPart], chapters: &[ChapterMark]) -> AppResult<ExportedFile> {
        if self.full_when_assembling.load(Ordering::SeqCst) {
            return Err(storage_full());
        }
        self.events.lock().unwrap().push(Event::Assemble {
            title: target.title.clone(),
            artist: target.artist.clone(),
            parts: parts.iter().map(|part| part.index).collect(),
            chapters: chapters.iter().map(|chapter| (chapter.title.clone(), chapter.start_ms)).collect(),
        });
        self.targets.lock().unwrap().push(target.clone());
        let path = PathBuf::from(format!("/exports/{}/{}.{}", target.document_id, target.file_stem, target.format.extension()));
        Ok(ExportedFile { path, size_bytes: 1 })
    }

    fn discard_parts(&self, _export_id: &ExportId) -> AppResult<()> {
        self.events.lock().unwrap().push(Event::Discard);
        Ok(())
    }

    fn parts_bytes(&self) -> AppResult<u64> {
        Ok(0)
    }

    fn delete_file(&self, path: &Path) -> AppResult<()> {
        self.deleted.lock().unwrap().push(path.to_path_buf());
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum Failure {
    Broken,
    Full,
}

struct FakeWriter {
    events: Arc<Mutex<Vec<Event>>>,
    failure: Option<Failure>,
    index: u32,
    path: PathBuf,
    sample_rate: u32,
    samples: u64,
    finished: bool,
}

impl ExportWriter for FakeWriter {
    fn append(&mut self, audio: &Audio) -> AppResult<()> {
        match self.failure {
            Some(Failure::Broken) => return Err(AppError::new(ErrorName::Internal).with_detail("fake encoder failure")),
            Some(Failure::Full) => return Err(storage_full()),
            None => {}
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

/// Keeps `1 / speed` of every push and records each stretcher's speed. `flush_tail` extra samples come out at each
/// part's end, like a real stretcher's small error.
#[derive(Default)]
pub(crate) struct ShorteningStretch {
    speeds: Mutex<Vec<u16>>,
    pub flush_tail: AtomicUsize,
}

impl ShorteningStretch {
    pub fn speeds(&self) -> Vec<u16> {
        self.speeds.lock().unwrap().clone()
    }
}

impl TimeStretch for ShorteningStretch {
    fn stretcher(&self, _sample_rate: u32, speed: ExportSpeed) -> Box<dyn TimeStretcher> {
        self.speeds.lock().unwrap().push(speed.percent());
        Box::new(Shortening { factor: speed.as_factor(), tail: self.flush_tail.load(Ordering::SeqCst) })
    }
}

struct Shortening {
    factor: f32,
    tail: usize,
}

impl TimeStretcher for Shortening {
    fn push(&mut self, samples: &[f32], out: &mut Vec<f32>) {
        out.extend(std::iter::repeat_n(0.0, (samples.len() as f32 / self.factor) as usize));
    }

    fn flush(&mut self, out: &mut Vec<f32>) {
        out.extend(std::iter::repeat_n(0.0, self.tail));
    }
}
