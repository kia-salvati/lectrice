//! Whole-book audio export (spec §4a): `StartAudioExport`, `GetAudioExport`, `ListAudioExports`, `CancelAudioExport`
//! and `GetAudioExportFile`, over the job registry (`ExportJobs`) and its runner.

mod audio_export_queries;
#[cfg(test)]
mod fakes;
mod jobs;
mod plan;
mod progress;
mod run;
mod start_audio_export;
#[cfg(test)]
mod tests;

pub use audio_export_queries::{CancelAudioExport, ExportDownload, GetAudioExport, GetAudioExportFile, ListAudioExports};
pub use jobs::{ExportDeps, ExportJobs, ExportSettings, NewExport};
pub use start_audio_export::{StartAudioExport, StartAudioExportArgs};
