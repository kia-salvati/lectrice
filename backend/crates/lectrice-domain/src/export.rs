use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::clock::Timestamp;
use crate::ids::{DocumentId, ExportId, VoiceId};

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioExport {
    pub id: ExportId,
    pub document_id: DocumentId,
    pub voice_id: VoiceId,
    pub format: ExportFormat,
    pub status: ExportStatus,
    pub sentences_done: u32,
    pub sentences_total: u32,
    /// Finished parts of about ten listening-minutes each (records saved before v1.1 have none).
    #[serde(default)]
    pub parts_done: u32,
    #[serde(default)]
    pub parts_total: u32,
    pub estimated_seconds_left: Option<u32>,
}

/// An export job as it is kept on disk, so finished exports are still listed after a restart.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRecord {
    pub export: AudioExport,
    pub read_skipped: bool,
    pub created_at: Timestamp,
    /// Set once the job is done.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<PathBuf>,
    /// The finished parts, in order. A resumed export continues after the last one.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parts: Vec<ExportPart>,
    /// The chapter marks inside the finished parts, measured from the start of the book.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub chapters: Vec<ChapterMark>,
}

/// One finished part of an export, as the exporter wrote it.
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

/// What a whole-book export writes: one file per document, voice and format, tagged with `title` and `artist`.
/// `export_id` names the job, so its parts can be kept apart from other jobs' parts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportTarget {
    pub export_id: ExportId,
    pub document_id: DocumentId,
    pub voice_id: VoiceId,
    pub format: ExportFormat,
    pub title: String,
    pub artist: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportedFile {
    pub path: PathBuf,
    pub size_bytes: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_round_trip_through_their_extension() {
        for format in ExportFormat::ALL {
            assert_eq!(ExportFormat::from_extension(format.extension()), Some(format));
            assert_eq!(serde_json::to_value(format).unwrap(), format.extension());
        }
        assert_eq!(ExportFormat::from_extension("wav"), None);
    }
}
