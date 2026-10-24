//! `FfmpegExporter`. The tests that run the real ffmpeg are `#[ignore]`d: `cargo test -p lectrice-infra -- --ignored`.

use std::path::{Path, PathBuf};
use std::process::Command;

use lectrice_domain::{
    Audio, AudioExporter, ChapterMark, DocumentId, ErrorName, ExportFormat, ExportId, ExportPart, ExportSpeed, ExportTarget, TimeStretch, VoiceId,
};
use lectrice_infra::{FfmpegExporter, SonicTimeStretch};
use serde_json::Value;
use tempfile::TempDir;

const SAMPLE_RATE: u32 = 24_000;
const TONE_HZ: f32 = 440.0;
const EXPORT_ID: &str = "e1";

fn target(format: ExportFormat) -> ExportTarget {
    ExportTarget {
        export_id: ExportId::new(EXPORT_ID),
        document_id: DocumentId::parse(&format!("{:064x}", 5)).unwrap(),
        voice_id: VoiceId::new("af_heart"),
        format,
        engine_id: "engine-a".into(),
        speed: ExportSpeed::NORMAL,
        range: None,
        file_stem: "af_heart".into(),
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

fn parts_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("exports").join("parts").join(EXPORT_ID)
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

fn write_part(exporter: &FfmpegExporter, format: ExportFormat, index: u32, clips: &[Audio]) -> ExportPart {
    let mut writer = exporter.begin_part(&target(format), index, SAMPLE_RATE).unwrap();
    for clip in clips {
        writer.append(clip).unwrap();
    }
    writer.finish().unwrap()
}

/// Three one-second clips in two parts, a chapter at the start and one at 2 s.
fn export(data_dir: &Path, format: ExportFormat) -> PathBuf {
    let exporter = FfmpegExporter::new("ffmpeg", data_dir);
    exporter.check_available().unwrap();
    let parts = [write_part(&exporter, format, 0, &[tone(1.0), tone(1.0)]), write_part(&exporter, format, 1, &[tone(1.0)])];
    let chapters = [ChapterMark { title: "Chapter 1. Start".into(), start_ms: 0 }, ChapterMark { title: "Chapter 2. Next".into(), start_ms: 2000 }];
    let file = exporter.assemble(&target(format), &parts, &chapters).unwrap();
    assert!(file.size_bytes > 0);
    assert_eq!(files_in(&parts_dir(data_dir)), ["part-0000.wav", "part-0001.wav"], "assembling keeps the parts");
    exporter.discard_parts(&ExportId::new(EXPORT_ID)).unwrap();
    file.path
}

#[test]
fn parts_are_wav_files_named_by_index_and_can_be_discarded() {
    let data_dir = TempDir::new().unwrap();
    let exporter = FfmpegExporter::new("/nonexistent/ffmpeg", data_dir.path());

    let part = write_part(&exporter, ExportFormat::M4b, 3, &[tone(0.5)]);

    assert_eq!((part.index, part.samples, part.sample_rate), (3, u64::from(SAMPLE_RATE / 2), SAMPLE_RATE));
    assert_eq!(part.path, parts_dir(data_dir.path()).join("part-0003.wav"));
    let bytes = std::fs::read(&part.path).unwrap();
    assert_eq!((&bytes[..4], &bytes[8..12]), (&b"RIFF"[..], &b"WAVE"[..]));
    exporter.discard_parts(&ExportId::new(EXPORT_ID)).unwrap();
    assert!(!parts_dir(data_dir.path()).exists());
    exporter.discard_parts(&ExportId::new(EXPORT_ID)).unwrap();
}

#[test]
fn an_abandoned_part_leaves_nothing() {
    let data_dir = TempDir::new().unwrap();
    let exporter = FfmpegExporter::new("ffmpeg", data_dir.path());
    let mut writer = exporter.begin_part(&target(ExportFormat::M4b), 0, SAMPLE_RATE).unwrap();
    writer.append(&tone(1.0)).unwrap();

    drop(writer);

    assert!(files_in(&parts_dir(data_dir.path())).is_empty());
}

#[test]
fn a_missing_program_is_exporter_unavailable() {
    let data_dir = TempDir::new().unwrap();
    let exporter = FfmpegExporter::new("/nonexistent/ffmpeg", data_dir.path());
    let part = write_part(&exporter, ExportFormat::Mp3, 0, &[tone(0.1)]);

    assert_eq!(exporter.check_available().unwrap_err().primary(), ErrorName::ExporterUnavailable);
    assert_eq!(exporter.assemble(&target(ExportFormat::Mp3), &[part], &[]).unwrap_err().primary(), ErrorName::ExporterUnavailable);
}

#[test]
fn leftover_partial_files_are_removed_and_finished_parts_stay() {
    let data_dir = TempDir::new().unwrap();
    let directory = data_dir.path().join("exports").join(format!("{:064x}", 5));
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join(".partial-abc.mp3"), b"x").unwrap();
    std::fs::write(directory.join("af_heart.mp3"), b"x").unwrap();
    let parts = parts_dir(data_dir.path());
    std::fs::create_dir_all(&parts).unwrap();
    std::fs::write(parts.join(".partial-def.wav"), b"x").unwrap();
    std::fs::write(parts.join("part-0000.wav"), b"x").unwrap();

    let removed = FfmpegExporter::new("ffmpeg", data_dir.path()).remove_partial_files().unwrap();

    assert_eq!(removed, 2);
    assert_eq!(files_in(&directory), ["af_heart.mp3"]);
    assert_eq!(files_in(&parts), ["part-0000.wav"]);
}

#[test]
fn parts_bytes_count_every_part_and_only_export_files_are_deleted() {
    let data_dir = TempDir::new().unwrap();
    let exporter = FfmpegExporter::new("ffmpeg", data_dir.path());
    let part = write_part(&exporter, ExportFormat::M4b, 0, &[tone(0.5)]);

    assert_eq!(exporter.parts_bytes().unwrap(), std::fs::metadata(&part.path).unwrap().len());

    let file = data_dir.path().join("exports").join(format!("{:064x}", 5)).join("af_heart.m4b");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, b"x").unwrap();
    exporter.delete_file(&file).unwrap();
    assert!(!file.exists());
    exporter.delete_file(&file).unwrap();
    let outside = data_dir.path().join("library.json");
    std::fs::write(&outside, b"x").unwrap();
    assert_eq!(exporter.delete_file(&outside).unwrap_err().primary(), ErrorName::Internal);
    let climbing = data_dir.path().join("exports").join("..").join("library.json");
    assert_eq!(exporter.delete_file(&climbing).unwrap_err().primary(), ErrorName::Internal);
    assert!(outside.exists());
}

/// C22: ffmpeg reports a full disk only in its error text.
#[cfg(unix)]
#[test]
fn an_ffmpeg_that_runs_out_of_space_is_storage_full() {
    use std::os::unix::fs::PermissionsExt;
    let data_dir = TempDir::new().unwrap();
    let program = data_dir.path().join("full-ffmpeg");
    std::fs::write(&program, "#!/bin/sh\necho 'af_heart.m4b: No space left on device' >&2\nexit 1\n").unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
    let exporter = FfmpegExporter::new(&program, data_dir.path());
    let part = write_part(&exporter, ExportFormat::M4b, 0, &[tone(0.1)]);

    let error = exporter.assemble(&target(ExportFormat::M4b), &[part], &[]).unwrap_err();

    assert_eq!(error.names(), [ErrorName::StorageFull, ErrorName::StorageFailed]);
}

/// C10 on the desktop: 3 s of 1x audio stretched to 1.5x in two parts lasts 2 s, and the file is named after it.
#[test]
#[ignore = "runs the system ffmpeg and ffprobe"]
fn a_baked_in_speed_shortens_the_file() {
    let data_dir = TempDir::new().unwrap();
    let exporter = FfmpegExporter::new("ffmpeg", data_dir.path());
    let fast = ExportSpeed::from_percent(150).unwrap();
    let fast_target = ExportTarget { speed: fast, file_stem: "af_heart.engine-a.150".into(), ..target(ExportFormat::M4b) };
    let stretched_part = |index: u32, seconds: f32| {
        let mut stretcher = SonicTimeStretch.stretcher(SAMPLE_RATE, fast);
        let mut samples = Vec::new();
        stretcher.push(&tone(seconds).samples, &mut samples);
        stretcher.flush(&mut samples);
        let mut writer = exporter.begin_part(&fast_target, index, SAMPLE_RATE).unwrap();
        writer.append(&Audio { samples, sample_rate: SAMPLE_RATE }).unwrap();
        writer.finish().unwrap()
    };
    let parts = [stretched_part(0, 2.0), stretched_part(1, 1.0)];

    let file = exporter.assemble(&fast_target, &parts, &[]).unwrap();

    assert!(file.path.ends_with("af_heart.engine-a.150.m4b"));
    let seconds = duration(&ffprobe(&file.path));
    assert!((seconds - 2.0).abs() <= 2.0 * 0.02, "duration {seconds}");
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
