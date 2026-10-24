//! Audio export (spec §4a): starting, estimating, pausing, resuming, cancelling, listing and deleting exports, over
//! the job registry (`ExportJobs`) and its runner, which writes each book as resumable parts.

mod audio_export_queries;
mod estimate;
#[cfg(test)]
pub(crate) mod fakes;
mod jobs;
mod plan;
mod progress;
mod run;
mod start_audio_export;
#[cfg(test)]
mod tests;

pub use audio_export_queries::{
    CancelAudioExport, DeleteAudioExport, ExportDownload, GetAudioExport, GetAudioExportFile, ListAllAudioExports, ListAudioExports, MarkExportSaved,
    MarkExportSavedArgs, PauseAudioExport, PauseAudioExportArgs, PauseAudioExports, ResumeAudioExport, ResumeAudioExports,
};
pub use estimate::{EstimateAudioExport, EstimateAudioExportArgs, ExportEstimate, PartOffset, TocOffset};
pub use jobs::{ExportDeps, ExportJobs, ExportSettings, ExportSpeech, NewExport, PurgedExports};
pub use start_audio_export::{StartAudioExport, StartAudioExportArgs, StartAudioExportDeps};
