//! The Lectrice map an exported M4B carries (spec 23 §3.1) and matching an audiobook file to a book (§3.3).
//! Pure: the container format lives in `lectrice-infra` (`export::mp4_map`).

use std::collections::{HashMap, HashSet};
use std::ops::RangeInclusive;

use serde::{Deserialize, Serialize};

use crate::document::{Document, Sentence};
use crate::export::{ChapterMark, ExportRange, ExportSpeed};
use crate::ids::{DocumentId, SentenceId};

pub const MAP_FORMAT: &str = "lectrice-audiobook";
/// The map's major version; a file with a newer one is read as a foreign audiobook.
pub const MAP_VERSION: u32 = 1;
/// Remapping across cleaner versions keeps sentence follow-along when at least this share of the map is found.
const REMAP_MIN_PERCENT: u32 = 90;
/// How far ahead in the book a map sentence is looked for (the walk is monotone, so insertions are skipped over).
const REMAP_WINDOW: usize = 200;
/// Chapter mode needs this share of the file's chapters to match TOC titles to call it a match.
const CHAPTER_MIN_PERCENT: u32 = 50;
const PERCENT: u64 = 100;
/// The most sentences a map may hold (a very long book has well under 100 000).
pub const MAX_MAP_SENTENCES: usize = 200_000;
/// The speeds a file can have been made at: wider than today's export speeds so a later one still reads, never 0.
const MAP_SPEED_PERCENTS: RangeInclusive<u16> = 50..=300;
const MAP_SAMPLE_RATES: RangeInclusive<u32> = 8_000..=192_000;
/// 1 000 hours.
const MAX_MAP_DURATION_MS: u64 = 1_000 * 60 * 60 * 1_000;
const MAX_MAP_ID_BYTES: usize = 128;
const HASH_HEX_DIGITS: usize = 8;
const FNV_OFFSET: u32 = 0x811c_9dc5;
const FNV_PRIME: u32 = 0x0100_0193;

/// One spoken sentence in the file: its id, the hash of what was said and where it starts (ms on the file's timeline).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SentenceMark(pub SentenceId, pub String, pub u64);

impl SentenceMark {
    pub fn new(sentence: &Sentence, start_ms: u64) -> Self {
        Self(sentence.id, text_hash(sentence.spoken_text()), start_ms)
    }

    pub fn sentence_id(&self) -> SentenceId {
        self.0
    }

    pub fn hash(&self) -> &str {
        &self.1
    }

    pub fn start_ms(&self) -> u64 {
        self.2
    }
}

/// What an exported M4B says about itself. Serialized as the JSON payload of the file's `uuid` box.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LectriceMap {
    pub format: String,
    pub version: u32,
    pub document_id: DocumentId,
    pub cleaner_version: u32,
    pub engine_id: String,
    pub voice_id: String,
    pub speed_percent: u16,
    pub read_skipped: bool,
    #[serde(default)]
    pub range: Option<ExportRange>,
    pub sample_rate: u32,
    pub duration_ms: u64,
    pub sentences: Vec<SentenceMark>,
}

impl LectriceMap {
    /// Ours, and a version this build understands.
    pub fn is_supported(&self) -> bool {
        self.format == MAP_FORMAT && self.version == MAP_VERSION
    }

    /// A map read from a file is a claim, not a fact (anyone can write one): `Err` names the first value that can't be
    /// true. Guarantees for the caller: a usable speed, sample rate and length, at most [`MAX_MAP_SENTENCES`] sentences,
    /// each id once, and start times that never go back or past the end.
    pub fn check(&self) -> Result<(), &'static str> {
        if !MAP_SPEED_PERCENTS.contains(&self.speed_percent) {
            return Err("speed");
        }
        if !MAP_SAMPLE_RATES.contains(&self.sample_rate) {
            return Err("sample rate");
        }
        if self.duration_ms == 0 || self.duration_ms > MAX_MAP_DURATION_MS {
            return Err("duration");
        }
        if !is_plain_id(&self.engine_id) || !is_plain_id(&self.voice_id) {
            return Err("engine or voice id");
        }
        if self.sentences.len() > MAX_MAP_SENTENCES {
            return Err("too many sentences");
        }
        let mut seen = HashSet::with_capacity(self.sentences.len());
        let mut previous_start = 0;
        for mark in &self.sentences {
            if mark.hash().len() != HASH_HEX_DIGITS || !mark.hash().bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return Err("sentence hash");
            }
            if mark.start_ms() < previous_start || mark.start_ms() > self.duration_ms {
                return Err("sentence start");
            }
            if !seen.insert(mark.sentence_id()) {
                return Err("sentence id twice");
            }
            previous_start = mark.start_ms();
        }
        Ok(())
    }
}

/// Engine and voice ids are short ASCII names; anything else in a file's map is refused before it reaches a log or a screen.
fn is_plain_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= MAX_MAP_ID_BYTES && id.bytes().all(|byte| byte.is_ascii_graphic())
}

/// FNV-1a 32 of the lowercased text with whitespace collapsed, as 8 hex digits: names a sentence across cleaner
/// versions without storing its text.
pub fn text_hash(text: &str) -> String {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
    let hash = normalized.bytes().fold(FNV_OFFSET, |hash, byte| (hash ^ u32::from(byte)).wrapping_mul(FNV_PRIME));
    format!("{hash:08x}")
}

/// What `read_audiobook` found in a file: its map (if Lectrice made it), length, chapter marks and title tag.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AudiobookInfo {
    pub map: Option<LectriceMap>,
    pub duration_ms: u64,
    pub chapters: Vec<ChapterMark>,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudiobookMode {
    /// The map matched this book's sentences (the same cleaner version, or remapped).
    Sentences,
    /// Only chapter starts are known.
    Chapters,
    /// Nothing in the text can follow the file.
    Unaligned,
}

/// A sentence's place in the file. `end_ms` is where the next sentence starts (the file's end for the last).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimedSentence {
    pub sentence_id: SentenceId,
    pub start_ms: u64,
    pub end_ms: u64,
}

/// A chapter start in the file, tied to the book's TOC entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimedChapter {
    pub toc_index: u32,
    pub sentence_id: SentenceId,
    pub start_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudiobookTimeline {
    pub mode: AudiobookMode,
    /// Remapped from another cleaner version (sentence mode only).
    pub remapped: bool,
    pub matched_percent: u32,
    /// The speed baked into the file; 100 for a foreign file.
    pub file_speed_percent: u16,
    pub duration_ms: u64,
    pub voice_id: Option<String>,
    pub sentences: Vec<TimedSentence>,
    pub chapters: Vec<TimedChapter>,
}

/// Why a file can't be tied to this book.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudiobookMismatch {
    /// Its map names another book (the shell offers to open that one).
    OtherBook(DocumentId),
}

/// Spec 23 §3.3. A map of this book gives sentence follow-along (remapped across cleaner versions when ≥ 90 % is
/// found); anything else falls back to chapter titles.
pub fn match_audiobook(document: &Document, info: &AudiobookInfo) -> Result<AudiobookTimeline, AudiobookMismatch> {
    let Some(map) = info.map.as_ref().filter(|map| map.is_supported() && map.check().is_ok()) else {
        return Ok(chapter_timeline(document, info, None));
    };
    if map.document_id != document.id {
        return Err(AudiobookMismatch::OtherBook(map.document_id.clone()));
    }
    let duration_ms = if map.duration_ms > 0 { map.duration_ms } else { info.duration_ms };
    let book = sentences_in_order(document);
    let exact = map.cleaner_version == document.cleaner_version;
    let matched = if exact { exact_match(&book, map) } else { None }.unwrap_or_else(|| remap(&book, map));
    let percent = share(matched.len(), map.sentences.len());
    if percent < REMAP_MIN_PERCENT || matched.is_empty() {
        return Ok(chapter_timeline(document, info, Some(map)));
    }
    let remapped = !(exact && matched.len() == map.sentences.len());
    Ok(AudiobookTimeline {
        mode: AudiobookMode::Sentences,
        remapped,
        matched_percent: percent,
        file_speed_percent: map.speed_percent,
        duration_ms,
        voice_id: Some(map.voice_id.clone()),
        sentences: timed(&matched, map, duration_ms),
        chapters: Vec::new(),
    })
}

/// Every sentence a playback order can contain (block sentences, then the block's cue), in reading order.
fn sentences_in_order(document: &Document) -> Vec<&Sentence> {
    document.blocks.iter().flat_map(|block| block.sentences.iter().chain(block.cue.iter())).collect()
}

/// (index in the map, the book's sentence id) when every id exists with the same hash.
fn exact_match(book: &[&Sentence], map: &LectriceMap) -> Option<Vec<(usize, SentenceId)>> {
    let hashes: HashMap<SentenceId, String> = book.iter().map(|sentence| (sentence.id, text_hash(sentence.spoken_text()))).collect();
    map.sentences.iter().enumerate().map(|(index, mark)| (hashes.get(&mark.sentence_id())? == mark.hash()).then_some((index, mark.sentence_id()))).collect()
}

/// Walks the map in order and finds each hash at or after the last match, within [REMAP_WINDOW] sentences.
fn remap(book: &[&Sentence], map: &LectriceMap) -> Vec<(usize, SentenceId)> {
    let hashes: Vec<String> = book.iter().map(|sentence| text_hash(sentence.spoken_text())).collect();
    let mut next = 0;
    let mut matched = Vec::new();
    for (index, mark) in map.sentences.iter().enumerate() {
        let end = (next + REMAP_WINDOW).min(hashes.len());
        if let Some(found) = (next..end).find(|&position| hashes[position] == mark.hash()) {
            matched.push((index, book[found].id));
            next = found + 1;
        }
    }
    matched
}

/// A matched sentence runs until the next map entry starts (matched or not), so a dropped sentence's audio isn't
/// glued to its neighbour.
fn timed(matched: &[(usize, SentenceId)], map: &LectriceMap, duration_ms: u64) -> Vec<TimedSentence> {
    matched
        .iter()
        .map(|&(index, sentence_id)| {
            let start_ms = map.sentences[index].start_ms();
            let end_ms = map.sentences.get(index + 1).map_or(duration_ms, SentenceMark::start_ms).max(start_ms);
            TimedSentence { sentence_id, start_ms, end_ms }
        })
        .collect()
}

fn chapter_timeline(document: &Document, info: &AudiobookInfo, map: Option<&LectriceMap>) -> AudiobookTimeline {
    let mut chapters = Vec::new();
    let mut toc_from = 0;
    for mark in &info.chapters {
        let wanted = title_key(&mark.title);
        if wanted.is_empty() {
            continue;
        }
        let found = document.toc.iter().enumerate().skip(toc_from).find(|(_, entry)| title_key(&entry.title) == wanted);
        if let Some((index, entry)) = found {
            chapters.push(TimedChapter { toc_index: u32::try_from(index).unwrap_or(u32::MAX), sentence_id: entry.sentence_id, start_ms: mark.start_ms });
            toc_from = index + 1;
        }
    }
    let percent = share(chapters.len(), info.chapters.len());
    let mode = if chapters.is_empty() { AudiobookMode::Unaligned } else { AudiobookMode::Chapters };
    AudiobookTimeline {
        mode,
        remapped: false,
        matched_percent: percent,
        file_speed_percent: map.map_or(ExportSpeed::NORMAL.percent(), |map| map.speed_percent),
        duration_ms: info.duration_ms,
        voice_id: map.map(|map| map.voice_id.clone()),
        sentences: Vec::new(),
        chapters,
    }
}

/// Whether the file's chapters fit the book well enough to suggest it (the "which book" list).
pub fn chapters_match_well(timeline: &AudiobookTimeline) -> bool {
    timeline.mode == AudiobookMode::Chapters && timeline.matched_percent >= CHAPTER_MIN_PERCENT
}

/// Letters only, lowercased, accents and leading numbering dropped: "3. The Winter Storm" ~ "the winter storm".
fn title_key(title: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    let folded: String = title.nfkd().filter(char::is_ascii).collect::<String>().to_lowercase();
    folded.split(|c: char| !c.is_ascii_alphabetic()).filter(|word| !word.is_empty() && *word != "chapter").collect::<Vec<_>>().join(" ")
}

fn share(part: usize, whole: usize) -> u32 {
    if whole == 0 {
        return 0;
    }
    u32::try_from(part as u64 * PERCENT / whole as u64).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Block, BlockKind, TocEntry};

    fn sentence(id: u32, text: &str) -> Sentence {
        Sentence { id: SentenceId(id), text: text.into(), page_index: 0, speech_text: None }
    }

    fn document(cleaner_version: u32, texts: &[&str]) -> Document {
        let sentences = texts.iter().enumerate().map(|(id, text)| sentence(u32::try_from(id).unwrap(), text)).collect();
        Document {
            id: DocumentId::parse(&"ab".repeat(32)).unwrap(),
            title: "The Long Book".into(),
            source_file_name: "long.pdf".into(),
            page_count: 1,
            cleaner_version,
            pages: Vec::new(),
            toc: vec![
                TocEntry { id: 0, title: "1 The Harbour".into(), level: 1, sentence_id: SentenceId(0), page_index: 0 },
                TocEntry { id: 1, title: "2 The Lamp".into(), level: 1, sentence_id: SentenceId(2), page_index: 0 },
            ],
            blocks: vec![Block { id: 0, kind: BlockKind::Paragraph, level: None, page_index: 0, spoken: true, text: None, sentences, cue: None }],
        }
    }

    fn map_of(document: &Document) -> LectriceMap {
        let sentences = document.blocks[0].sentences.iter().enumerate().map(|(index, s)| SentenceMark::new(s, index as u64 * 1000)).collect();
        LectriceMap {
            format: MAP_FORMAT.into(),
            version: MAP_VERSION,
            document_id: document.id.clone(),
            cleaner_version: document.cleaner_version,
            engine_id: "kokoro-test".into(),
            voice_id: "af_heart".into(),
            speed_percent: 150,
            read_skipped: false,
            range: None,
            sample_rate: 24_000,
            duration_ms: 4_500,
            sentences,
        }
    }

    const TEXTS: [&str; 4] = ["The boat came in.", "Gulls  circled the mast.", "The lamp was lit.", "Night fell."];

    #[test]
    fn hash_ignores_case_and_spacing() {
        assert_eq!(text_hash("Gulls  circled\nthe mast."), text_hash("gulls circled the mast."));
        assert_ne!(text_hash("Night fell."), text_hash("Night falls."));
        assert_eq!(text_hash("").len(), 8);
    }

    #[test]
    fn same_book_same_cleaner_follows_every_sentence() {
        let book = document(4, &TEXTS);
        let info = AudiobookInfo { map: Some(map_of(&book)), duration_ms: 4_500, ..Default::default() };
        let timeline = match_audiobook(&book, &info).unwrap();
        assert_eq!(timeline.mode, AudiobookMode::Sentences);
        assert!(!timeline.remapped);
        assert_eq!(timeline.file_speed_percent, 150);
        assert_eq!(timeline.sentences.last().unwrap(), &TimedSentence { sentence_id: SentenceId(3), start_ms: 3_000, end_ms: 4_500 });
    }

    #[test]
    fn a_new_cleaner_version_is_remapped_by_hash() {
        let old = document(3, &TEXTS);
        let mut newer = document(4, &["A heading.", TEXTS[0], TEXTS[1], TEXTS[2], TEXTS[3]]);
        newer.id = old.id.clone();
        let info = AudiobookInfo { map: Some(map_of(&old)), ..Default::default() };
        let timeline = match_audiobook(&newer, &info).unwrap();
        assert_eq!(timeline.mode, AudiobookMode::Sentences);
        assert!(timeline.remapped);
        assert_eq!(timeline.matched_percent, 100);
        assert_eq!(timeline.sentences[0].sentence_id, SentenceId(1));
        assert_eq!(timeline.sentences[0].end_ms, 1_000);
    }

    #[test]
    fn too_few_matches_fall_back_to_chapters() {
        let old = document(3, &TEXTS);
        let mut changed = document(4, &["Other words.", "More other words.", "The lamp was lit.", "Different end."]);
        changed.id = old.id.clone();
        let chapters = vec![ChapterMark { title: "1 The Harbour".into(), start_ms: 0 }, ChapterMark { title: "2. the lamp".into(), start_ms: 2_000 }];
        let info = AudiobookInfo { map: Some(map_of(&old)), duration_ms: 4_500, chapters, title: None };
        let timeline = match_audiobook(&changed, &info).unwrap();
        assert_eq!(timeline.mode, AudiobookMode::Chapters);
        assert_eq!(timeline.chapters.len(), 2);
        assert_eq!(timeline.chapters[1].sentence_id, SentenceId(2));
        assert_eq!(timeline.file_speed_percent, 150);
    }

    #[test]
    fn a_map_of_another_book_is_refused() {
        let book = document(4, &TEXTS);
        let mut map = map_of(&book);
        map.document_id = DocumentId::parse(&"cd".repeat(32)).unwrap();
        let info = AudiobookInfo { map: Some(map.clone()), ..Default::default() };
        assert_eq!(match_audiobook(&book, &info), Err(AudiobookMismatch::OtherBook(map.document_id)));
    }

    #[test]
    fn a_foreign_file_without_matching_chapters_is_unaligned() {
        let book = document(4, &TEXTS);
        let info = AudiobookInfo { chapters: vec![ChapterMark { title: "Track 1".into(), start_ms: 0 }], duration_ms: 9_000, ..Default::default() };
        let timeline = match_audiobook(&book, &info).unwrap();
        assert_eq!(timeline.mode, AudiobookMode::Unaligned);
        assert_eq!(timeline.file_speed_percent, 100);
        assert!(!chapters_match_well(&timeline));
    }

    #[test]
    fn a_map_whose_values_cant_be_true_is_refused() {
        let good = map_of(&document(4, &TEXTS));
        assert_eq!(good.check(), Ok(()));
        let broken = |change: fn(&mut LectriceMap)| {
            let mut map = good.clone();
            change(&mut map);
            map.check()
        };
        assert_eq!(broken(|map| map.speed_percent = 0), Err("speed"));
        assert_eq!(broken(|map| map.sample_rate = 0), Err("sample rate"));
        assert_eq!(broken(|map| map.duration_ms = 0), Err("duration"));
        assert_eq!(broken(|map| map.duration_ms = u64::MAX), Err("duration"));
        assert_eq!(broken(|map| map.voice_id = "af heart\n".into()), Err("engine or voice id"));
        assert_eq!(broken(|map| map.engine_id = "e".repeat(MAX_MAP_ID_BYTES + 1)), Err("engine or voice id"));
        assert_eq!(broken(|map| map.sentences[1].1 = "not-hex!".into()), Err("sentence hash"));
        assert_eq!(broken(|map| map.sentences[2].2 = 500), Err("sentence start"));
        assert_eq!(broken(|map| map.sentences[3].2 = 4_501), Err("sentence start"));
        assert_eq!(broken(|map| map.sentences[3].0 = SentenceId(0)), Err("sentence id twice"));
        assert_eq!(
            broken(|map| map.sentences = (0..=MAX_MAP_SENTENCES).map(|id| SentenceMark(SentenceId(u32::try_from(id).unwrap()), "00000000".into(), 0)).collect()),
            Err("too many sentences")
        );
    }

    #[test]
    fn a_map_that_fails_the_check_never_gives_sentence_times() {
        let book = document(4, &TEXTS);
        let mut map = map_of(&book);
        map.sentences.swap(1, 2);
        let info = AudiobookInfo { map: Some(map), duration_ms: 4_500, ..Default::default() };
        let timeline = match_audiobook(&book, &info).unwrap();
        assert_eq!(timeline.mode, AudiobookMode::Unaligned);
        assert!(timeline.sentences.is_empty());
    }

    #[test]
    fn the_map_round_trips_as_compact_json() {
        let map = map_of(&document(4, &TEXTS));
        let json = serde_json::to_string(&map).unwrap();
        assert!(json.contains(r#""sentences":[[0,""#));
        assert_eq!(serde_json::from_str::<LectriceMap>(&json).unwrap(), map);
    }
}
