//! `FfmpegExporter`: MP3 and M4B (with chapter marks) through the system ffmpeg.

mod ffmetadata;
mod ffmpeg;

pub use ffmpeg::FfmpegExporter;
