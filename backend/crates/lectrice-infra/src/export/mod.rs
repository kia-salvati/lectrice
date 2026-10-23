//! `FfmpegExporter`: WAV parts, joined into MP3 or M4B (with chapter marks) through the system ffmpeg.

mod ffmetadata;
mod ffmpeg;

pub use ffmpeg::FfmpegExporter;
