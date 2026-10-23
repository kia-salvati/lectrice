//! `FfmpegExporter`. The tests that run the real ffmpeg are `#[ignore]`d: `cargo test -p lectrice-infra -- --ignored`.

use std::path::Path;
use std::process::Command;

use lectrice_domain::{Audio, AudioExporter, DocumentId, ErrorName, ExportFormat, ExportTarget, VoiceId};
use lectrice_infra::FfmpegExporter;
use serde_json::Value;
use tempfile::TempDir;

const SAMPLE_RATE: u32 = 24_000;
const TONE_HZ: f32 = 440.0;

fn target(format: ExportFormat) -> ExportTarget {
    ExportTarget {
        document_id: DocumentId::parse(&format!("{:064x}", 5)).unwrap(),
        voice_id: VoiceId::new("af_heart"),
        format,
        title: "A Book; with = signs".into(),
        artist: "Heart".into(),
    }
}

fn tone(seconds: f32) -> Audio {
    let count = (seconds * SAMPLE_RATE as f32) as usize;
    let samples = (0..count).map(|index| 0.3 * (index as f32 * TONE_HZ * std::f32::consts::TAU / SAMPLE_RATE as f32).sin()).collect();
    Audio { samples, sample_rate: SAMPLE_RATE }
}

fn files_in(directory: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(directory).unwrap().map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    names
}

fn ffprobe(path: &Path) -> Value {
    let output = Command::new("ffprobe")
        .args(["-v", "error", "-print_format", "json", "-show_format", "-show_streams", "-show_chapters"])
        .arg(path)
        .output()
        .expect("ffprobe is installed");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    serde_json::from_slice(&output.stdout).unwrap()
}

fn duration(probe: &Value) -> f64 {
    probe["format"]["duration"].as_str().unwrap().parse().unwrap()
}

/// Three one-second clips, a chapter before the first and the third.
fn export(data_dir: &Path, format: ExportFormat) -> std::path::PathBuf {
    let exporter = FfmpegExporter::new("ffmpeg", data_dir);
    exporter.check_available().unwrap();
    let mut writer = exporter.begin(&target(format), SAMPLE_RATE).unwrap();
    writer.start_chapter("Chapter 1. Start").unwrap();
    writer.append(&tone(1.0)).unwrap();
    writer.append(&tone(1.0)).unwrap();
    writer.start_chapter("Chapter 2. Next").unwrap();
    writer.append(&tone(1.0)).unwrap();
    let file = writer.finish().unwrap();
    assert!(file.size_bytes > 0);
    file.path
}

#[test]
fn a_missing_program_is_exporter_unavailable() {
    let data_dir = TempDir::new().unwrap();
    let exporter = FfmpegExporter::new("/nonexistent/ffmpeg", data_dir.path());

    assert_eq!(exporter.check_available().unwrap_err().primary(), ErrorName::ExporterUnavailable);
    assert_eq!(exporter.begin(&target(ExportFormat::Mp3), SAMPLE_RATE).err().unwrap().primary(), ErrorName::ExporterUnavailable);
}

#[test]
fn leftover_partial_files_are_removed() {
    let data_dir = TempDir::new().unwrap();
    let directory = data_dir.path().join("exports").join(format!("{:064x}", 5));
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join(".partial-abc.mp3"), b"x").unwrap();
    std::fs::write(directory.join("af_heart.mp3"), b"x").unwrap();

    let removed = FfmpegExporter::new("ffmpeg", data_dir.path()).remove_partial_files().unwrap();

    assert_eq!(removed, 1);
    assert_eq!(files_in(&directory), ["af_heart.mp3"]);
}

#[test]
#[ignore = "runs the system ffmpeg and ffprobe"]
fn exports_a_mono_64k_mp3_with_tags() {
    let data_dir = TempDir::new().unwrap();

    let path = export(data_dir.path(), ExportFormat::Mp3);

    assert_eq!(path, data_dir.path().join("exports").join(format!("{:064x}", 5)).join("af_heart.mp3"));
    let probe = ffprobe(&path);
    let stream = &probe["streams"][0];
    assert_eq!((stream["codec_name"].as_str(), stream["channels"].as_u64(), stream["sample_rate"].as_str()), (Some("mp3"), Some(1), Some("24000")));
    assert!((duration(&probe) - 3.0).abs() < 0.15, "duration {}", duration(&probe));
    assert_eq!(probe["format"]["tags"]["title"], "A Book; with = signs");
    assert_eq!(probe["format"]["tags"]["artist"], "Heart");
    assert_eq!(files_in(path.parent().unwrap()), ["af_heart.mp3"], "no partial files are left");
}

#[test]
#[ignore = "runs the system ffmpeg and ffprobe"]
fn exports_an_aac_m4b_with_chapters() {
    let data_dir = TempDir::new().unwrap();

    let path = export(data_dir.path(), ExportFormat::M4b);

    let probe = ffprobe(&path);
    assert_eq!(probe["streams"][0]["codec_name"], "aac");
    assert!((duration(&probe) - 3.0).abs() < 0.15, "duration {}", duration(&probe));
    let chapters = probe["chapters"].as_array().unwrap();
    let titles: Vec<&str> = chapters.iter().map(|chapter| chapter["tags"]["title"].as_str().unwrap()).collect();
    assert_eq!(titles, ["Chapter 1. Start", "Chapter 2. Next"]);
    let second_start: f64 = chapters[1]["start_time"].as_str().unwrap().parse().unwrap();
    assert!((second_start - 2.0).abs() < 0.01, "second chapter starts at {second_start}");
    assert_eq!(probe["format"]["tags"]["title"], "A Book; with = signs");
    assert_eq!(files_in(path.parent().unwrap()), ["af_heart.m4b"], "no partial files are left");
}

#[test]
#[ignore = "runs the system ffmpeg"]
fn an_abandoned_export_stops_ffmpeg_and_leaves_nothing() {
    let data_dir = TempDir::new().unwrap();
    let exporter = FfmpegExporter::new("ffmpeg", data_dir.path());
    let mut writer = exporter.begin(&target(ExportFormat::M4b), SAMPLE_RATE).unwrap();
    writer.append(&tone(1.0)).unwrap();

    drop(writer);

    let directory = data_dir.path().join("exports").join(format!("{:064x}", 5));
    assert!(files_in(&directory).is_empty());
}
