//! `FfmpegExporter` (feature `ffmpeg-export`): WAV parts, joined into MP3 or M4B (with chapter marks) through the
//! system ffmpeg. `mp4_map` (always): the Lectrice map inside an M4B, shared by every shell's exporter.

#[cfg(feature = "ffmpeg-export")]
mod ffmetadata;
#[cfg(feature = "ffmpeg-export")]
mod ffmpeg;
pub mod mp4_map;

#[cfg(feature = "ffmpeg-export")]
pub use ffmpeg::FfmpegExporter;
