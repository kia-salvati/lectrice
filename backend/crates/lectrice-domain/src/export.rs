use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::clock::Timestamp;
use crate::error::{AppError, ErrorName};
use crate::ids::{DocumentId, ExportId, SentenceId, VoiceId};

const PERCENT: f32 = 100.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExportFormat {
    Mp3,
    M4b,
}

impl ExportFormat {
    pub const ALL: [ExportFormat; 2] = [ExportFormat::Mp3, ExportFormat::M4b];

    /// The file extension, also the format's name on the wire.
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Mp3 => "mp3",
            Self::M4b => "m4b",
        }
    }

    pub fn from_extension(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|format| format.extension() == value)
    }
}

/// The speed baked into an exported file, in percent. Only the player's steps are allowed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "u16", into = "u16")]
pub struct ExportSpeed(u16);

impl ExportSpeed {
    pub const NORMAL: Self = Self(100);
    pub const ALLOWED_PERCENTS: [u16; 6] = [75, 100, 125, 150, 175, 200];

    pub fn from_percent(percent: u16) -> Result<Self, AppError> {
        if Self::ALLOWED_PERCENTS.contains(&percent) {
            return Ok(Self(percent));
        }
        Err(AppError::new(ErrorName::RequestInvalid).with_detail(format!("export speed {percent} % is not one of {:?}", Self::ALLOWED_PERCENTS)))
    }

    pub const fn percent(self) -> u16 {
        self.0
    }

    pub fn as_factor(self) -> f32 {
        f32::from(self.0) / PERCENT
    }

    pub const fn is_normal(self) -> bool {
        self.0 == Self::NORMAL.0
    }
}

impl Default for ExportSpeed {
    fn default() -> Self {
        Self::NORMAL
    }
}

impl TryFrom<u16> for ExportSpeed {
    type Error = AppError;

    fn try_from(percent: u16) -> Result<Self, Self::Error> {
        Self::from_percent(percent)
    }
}

impl From<ExportSpeed> for u16 {
    fn from(speed: ExportSpeed) -> Self {
        speed.0
    }
}

/// The sentences a partial export covers, both ends included, in playback order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRange {
    pub from_sentence: SentenceId,
    pub to_sentence: SentenceId,
}

impl ExportRange {
    pub fn contains(&self, id: SentenceId) -> bool {
        (self.from_sentence..=self.to_sentence).contains(&id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExportStatus {
    Queued,
    Running,
    /// Stopped with its finished parts kept; resuming continues at the first unfinished part.
    Paused,
    Done,
    Failed,
    Cancelled,
}

impl ExportStatus {
    /// Queued or running: the job can still change.
    pub const fn is_active(self) -> bool {
        matches!(self, Self::Queued | Self::Running)
    }

    /// Active or paused: the job still owns its target file and its parts.
    pub const fn is_unfinished(self) -> bool {
        matches!(self, Self::Queued | Self::Running | Self::Paused)
    }
}

/// Why an export is paused. `System` and `EngineSwitch` pauses continue with "resume what you can"; the others wait
/// for the user (or for free space).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PauseReason {
    User,
    /// The app or the device stopped the work (also a stop or restart of the program).
    System,
    StorageFull,
    /// The other model bundle was needed for listening.
    EngineSwitch,
}

impl PauseReason {
    pub const fn resumes_by_itself(self) -> bool {
        matches!(self, Self::System | Self::EngineSwitch)
    }
}

/// How far an export got: writing its parts, or joining them into the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExportStage {
    #[default]
    Parts,
    Assemble,
}

/// Where a finished export's file is. `Shared` is an opaque handle the shell gave (e.g. a content URI); the core never
/// opens it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ExportLocation {
    AppStorage { path: PathBuf },
    Shared { uri: String },
}

/// Fields added after 1.0 have serde defaults, so older records load (spec 19 §5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioExport {
    pub id: ExportId,
    pub document_id: DocumentId,
    pub voice_id: VoiceId,
    pub format: ExportFormat,
    #[serde(rename = "speedPercent", default)]
    pub speed: ExportSpeed,
    /// The speech engine (model bundle) the export is made with. Empty in records saved before v0.2; the registry
    /// fills in the loaded engine when it loads them.
    #[serde(default)]
    pub engine_id: String,
    #[serde(default)]
    pub range: Option<ExportRange>,
    /// The shell's name for the range (e.g. "Ch. 7–12"), added to the title tag.
    #[serde(default)]
    pub range_label: Option<String>,
    /// The book's title when the export started, so it can be listed after the book is removed.
    #[serde(default)]
    pub title: String,
    pub status: ExportStatus,
    #[serde(default)]
    pub paused_by: Option<PauseReason>,
    #[serde(default)]
    pub stage: ExportStage,
    /// What stopped it: `StorageFull` while paused for space, any name when failed.
    #[serde(default)]
    pub error: Option<ErrorName>,
    pub sentences_done: u32,
    pub sentences_total: u32,
    /// Finished parts of about ten listening-minutes each (records saved before v0.2 have none).
    #[serde(default)]
    pub parts_done: u32,
    #[serde(default)]
    pub parts_total: u32,
    pub estimated_seconds_left: Option<u32>,
    /// Set when done.
    #[serde(default)]
    pub file_bytes: Option<u64>,
    /// Set when done.
    #[serde(default)]
    pub location: Option<ExportLocation>,
    /// Queued for a speech engine other than the loaded one. Worked out when the export is read; never stored as true.
    #[serde(default)]
    pub waiting_for_engine: bool,
}

/// An export job as it is kept on disk, so finished exports are still listed after a restart.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRecord {
    pub export: AudioExport,
    pub read_skipped: bool,
    pub created_at: Timestamp,
    /// Set once the job is done, while the file is in app storage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<PathBuf>,
    /// The finished parts, in order. A resumed export continues after the last one.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parts: Vec<ExportPart>,
    /// The chapter marks inside the finished parts, measured from the start of the export's output.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub chapters: Vec<ChapterMark>,
    /// The 1x samples that went into the finished parts (they hold fewer when the speed is baked in). Chapter marks are
    /// timed from it. 0 in records saved before v0.2, which were all made at 1x.
    #[serde(default)]
    pub source_samples: u64,
}

/// One finished part of an export, as the exporter wrote it. `samples` counts what the file holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportPart {
    pub index: u32,
    pub path: PathBuf,
    pub samples: u64,
    pub sample_rate: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterMark {
    pub title: String,
    pub start_ms: u64,
}

/// What an export writes: one file per document, voice, format, engine, speed and range, tagged with `title` and
/// `artist`. `export_id` names the job, so its parts can be kept apart from other jobs' parts. The file is named
/// `<file_stem>.<extension>` (see [`export_file_stem`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportTarget {
    pub export_id: ExportId,
    pub document_id: DocumentId,
    pub voice_id: VoiceId,
    pub format: ExportFormat,
    pub engine_id: String,
    pub speed: ExportSpeed,
    pub range: Option<ExportRange>,
    pub file_stem: String,
    pub title: String,
    pub artist: String,
}

/// `<voiceId>` for a whole-book 1x export when `short_names` is on (a shell with a single engine, so 1.0 file names
/// stay), else `<voiceId>.<engineId>.<speedPercent>`; a range adds `.s<from>-<to>`.
pub fn export_file_stem(voice_id: &VoiceId, engine_id: &str, speed: ExportSpeed, range: Option<ExportRange>, short_names: bool) -> String {
    let mut stem = if short_names && speed.is_normal() { voice_id.to_string() } else { format!("{voice_id}.{engine_id}.{}", speed.percent()) };
    if let Some(range) = range {
        stem.push_str(&format!(".s{}-{}", range.from_sentence, range.to_sentence));
    }
    stem
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportedFile {
    pub path: PathBuf,
    pub size_bytes: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn formats_round_trip_through_their_extension() {
        for format in ExportFormat::ALL {
            assert_eq!(ExportFormat::from_extension(format.extension()), Some(format));
            assert_eq!(serde_json::to_value(format).unwrap(), format.extension());
        }
        assert_eq!(ExportFormat::from_extension("wav"), None);
    }

    #[test]
    fn only_the_player_speeds_are_export_speeds() {
        for percent in ExportSpeed::ALLOWED_PERCENTS {
            assert_eq!(ExportSpeed::from_percent(percent).unwrap().percent(), percent);
        }
        for percent in [0, 76, 250] {
            assert_eq!(ExportSpeed::from_percent(percent).unwrap_err().primary(), ErrorName::RequestInvalid, "{percent}");
        }
        assert!((ExportSpeed::from_percent(150).unwrap().as_factor() - 1.5).abs() < f32::EPSILON);
        assert!(serde_json::from_value::<ExportSpeed>(json!(76)).is_err());
    }

    #[test]
    fn a_record_saved_by_1_1_loads_with_the_defaults() {
        let saved = json!({
            "export": {
                "id": "e1", "documentId": "a".repeat(64), "voiceId": "af_heart", "format": "m4b", "status": "paused",
                "sentencesDone": 2, "sentencesTotal": 4, "partsDone": 1, "partsTotal": 2, "estimatedSecondsLeft": null
            },
            "readSkipped": false, "createdAt": "2026-10-23T18:04:00Z"
        });

        let record: ExportRecord = serde_json::from_value(saved).unwrap();

        let export = &record.export;
        assert_eq!((export.speed, export.engine_id.as_str(), export.range, export.title.as_str()), (ExportSpeed::NORMAL, "", None, ""));
        assert_eq!((export.paused_by, export.stage, export.error, export.file_bytes, &export.location), (None, ExportStage::Parts, None, None, &None));
        assert_eq!(record.source_samples, 0);
        assert_eq!(serde_json::to_value(&record).unwrap()["export"]["speedPercent"], 100);
    }

    #[test]
    fn file_stems_name_the_engine_speed_and_range_unless_short() {
        let voice = VoiceId::new("af_heart");
        let range = ExportRange { from_sentence: SentenceId(12), to_sentence: SentenceId(340) };
        let fast = ExportSpeed::from_percent(150).unwrap();

        assert_eq!(export_file_stem(&voice, "e-int8", ExportSpeed::NORMAL, None, true), "af_heart");
        assert_eq!(export_file_stem(&voice, "e-int8", ExportSpeed::NORMAL, Some(range), true), "af_heart.s12-340");
        assert_eq!(export_file_stem(&voice, "e-int8", fast, None, true), "af_heart.e-int8.150");
        assert_eq!(export_file_stem(&voice, "e-int8", ExportSpeed::NORMAL, None, false), "af_heart.e-int8.100");
        assert_eq!(export_file_stem(&voice, "e-int8", fast, Some(range), false), "af_heart.e-int8.150.s12-340");
    }

    #[test]
    fn locations_are_tagged() {
        let shared = ExportLocation::Shared { uri: "content://media/1".into() };
        assert_eq!(serde_json::to_value(&shared).unwrap(), json!({ "kind": "shared", "uri": "content://media/1" }));
    }
}
