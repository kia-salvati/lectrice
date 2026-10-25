use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{self, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};

use hound::WavWriter;
use lectrice_domain::{
    AppError, AppResult, Audio, AudioExporter, ChapterMark, ErrorName, ExportFormat, ExportId, ExportPart, ExportTarget, ExportWriter, ExportedFile,
};
use tempfile::NamedTempFile;

use super::ffmetadata::{Tags, ffmetadata};
use crate::speech::{pcm16_mono, to_pcm16};
use crate::storage::{safe_component, storage_failed};

const EXPORTS_DIR: &str = "exports";
const PARTS_DIR: &str = "parts";
const PART_EXTENSION: &str = "wav";
const LIST_EXTENSION: &str = "ffconcat";
const METADATA_EXTENSION: &str = "txt";
const LOG_EXTENSION: &str = "log";
/// Partial files start with this, so a crash's leftovers can be found and removed.
const PARTIAL_PREFIX: &str = ".partial-";
const CONCAT_HEADER: &str = "ffconcat version 1.0\n";
const BITRATE: &str = "64k";
const MONO: &str = "1";
const GENRE: &str = "Audiobook";
const MILLIS_PER_SECOND: u64 = 1000;
/// How much of ffmpeg's error output goes into an error detail.
const STDERR_TAIL_BYTES: u64 = 2000;
/// `strerror(ENOSPC)`, as ffmpeg prints it.
const NO_SPACE_MESSAGE: &str = "No space left on device";

/// Parts are 16-bit mono WAV files in `<data_dir>/exports/parts/<exportId>/part-<n>.wav`. `assemble` joins them with
/// the system `ffmpeg` (concat demuxer) into MP3 (libmp3lame) or M4B (aac, with chapter marks) at
/// `<data_dir>/exports/<docId>/<fileStem>.<ext>`. Every file is written to a partial file first and renamed when done.
pub struct FfmpegExporter {
    program: PathBuf,
    exports_dir: PathBuf,
}

impl FfmpegExporter {
    /// `program` is a name looked up on `PATH` (e.g. `ffmpeg`) or a path.
    pub fn new(program: impl Into<PathBuf>, data_dir: &Path) -> Self {
        Self { program: program.into(), exports_dir: data_dir.join(EXPORTS_DIR) }
    }

    /// Deletes partial files left by exports that never finished (e.g. the server was killed). Returns how many.
    /// Finished parts stay: a paused export needs them.
    pub fn remove_partial_files(&self) -> AppResult<usize> {
        let mut directories = subdirectories(&self.exports_dir)?;
        directories.extend(subdirectories(&self.exports_dir.join(PARTS_DIR))?);
        let mut removed = 0;
        for directory in directories {
            for entry in fs::read_dir(&directory).map_err(|error| storage_failed("listing", &directory, error))? {
                let path = entry.map_err(|error| storage_failed("listing", &directory, error))?.path();
                let is_partial = path.file_name().and_then(|name| name.to_str()).is_some_and(|name| name.starts_with(PARTIAL_PREFIX));
                if is_partial && path.is_file() {
                    fs::remove_file(&path).map_err(|error| storage_failed("deleting", &path, error))?;
                    removed += 1;
                }
            }
        }
        Ok(removed)
    }

    fn parts_dir(&self, export_id: &ExportId) -> AppResult<PathBuf> {
        Ok(self.exports_dir.join(PARTS_DIR).join(safe_component(export_id.as_str())?))
    }

    /// ffmpeg reports a full disk only in its error text, so that is where `StorageFull` is found.
    fn encoder_failed(&self, what: &str, status: Option<std::process::ExitStatus>, stderr_log: &mut NamedTempFile) -> AppError {
        let status = status.map_or_else(|| "unknown".to_string(), |status| status.to_string());
        let tail = stderr_tail(stderr_log);
        let error = if tail.contains(NO_SPACE_MESSAGE) {
            AppError::new(ErrorName::StorageFull).and(ErrorName::StorageFailed)
        } else {
            AppError::new(ErrorName::Internal)
        };
        error.with_detail(format!("{} {what} (exit {status}): {tail}", self.program.display()))
    }
}

impl AudioExporter for FfmpegExporter {
    fn check_available(&self) -> AppResult<()> {
        let status = ffmpeg_command(&self.program).arg("-version").stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status();
        match status {
            Ok(status) if status.success() => Ok(()),
            Ok(status) => Err(AppError::new(ErrorName::ExporterUnavailable).with_detail(format!("{} -version exited with {status}", self.program.display()))),
            Err(error) => Err(spawn_failed(&self.program, error)),
        }
    }

    fn begin_part(&self, target: &ExportTarget, part_index: u32, sample_rate: u32) -> AppResult<Box<dyn ExportWriter>> {
        let directory = self.parts_dir(&target.export_id)?;
        fs::create_dir_all(&directory).map_err(|error| storage_failed("creating", &directory, error))?;
        let temp = partial_file(&directory, PART_EXTENSION)?;
        let file = temp.reopen().map_err(|error| storage_failed("opening", temp.path(), error))?;
        let wav = WavWriter::new(BufWriter::new(file), pcm16_mono(sample_rate)).map_err(|error| storage_failed("writing", temp.path(), error))?;
        let path = directory.join(format!("part-{part_index:04}.{PART_EXTENSION}"));
        Ok(Box::new(WavPartWriter { index: part_index, path, sample_rate, samples: 0, wav, temp }))
    }

    fn assemble(&self, target: &ExportTarget, parts: &[ExportPart], chapters: &[ChapterMark]) -> AppResult<ExportedFile> {
        let audible: Vec<&ExportPart> = parts.iter().filter(|part| part.samples > 0).collect();
        let Some(sample_rate) = audible.first().map(|part| part.sample_rate) else {
            return Err(AppError::new(ErrorName::Internal).with_detail("the export has no audio to join"));
        };
        if audible.iter().any(|part| part.sample_rate != sample_rate) {
            return Err(AppError::new(ErrorName::Internal).with_detail("the export parts have different sample rates"));
        }
        let total_ms = audible.iter().map(|part| part.samples).sum::<u64>() * MILLIS_PER_SECOND / u64::from(sample_rate.max(1));

        let directory = self.exports_dir.join(target.document_id.as_str());
        let target_path = directory.join(format!("{}.{}", safe_component(&target.file_stem)?, target.format.extension()));
        fs::create_dir_all(&directory).map_err(|error| storage_failed("creating", &directory, error))?;
        let list = write_partial(&directory, LIST_EXTENSION, &concat_list(&audible)?)?;
        let mut stderr_log = partial_file(&directory, LOG_EXTENSION)?;
        let output = partial_file(&directory, target.format.extension())?;

        let mut command = ffmpeg_command(&self.program);
        command.args(quiet_args()).args(["-f", "concat", "-safe", "0", "-i"]).arg(list.path());
        // Kept until ffmpeg is done with it.
        let _metadata = match target.format {
            ExportFormat::Mp3 => {
                command.args(["-map", "0:a"]).args(mp3_args(target));
                None
            }
            ExportFormat::M4b => {
                let tags = Tags { title: &target.title, artist: &target.artist };
                let metadata = write_partial(&directory, METADATA_EXTENSION, &ffmetadata(&tags, chapters, total_ms))?;
                command.args(["-f", "ffmetadata", "-i"]).arg(metadata.path()).args(m4b_args());
                Some(metadata)
            }
        };
        let stderr = stderr_log.reopen().map_err(|error| storage_failed("opening", stderr_log.path(), error))?;
        let status = command
            .arg("-y")
            .arg(output.path())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(stderr)
            .status()
            .map_err(|error| spawn_failed(&self.program, error))?;
        if !status.success() {
            return Err(self.encoder_failed("failed to join the parts", Some(status), &mut stderr_log));
        }
        output.persist(&target_path).map_err(|error| storage_failed("renaming", &target_path, error.error))?;
        let size_bytes = fs::metadata(&target_path).map_err(|error| storage_failed("inspecting", &target_path, error))?.len();
        Ok(ExportedFile { path: target_path, size_bytes })
    }

    fn discard_parts(&self, export_id: &ExportId) -> AppResult<()> {
        let directory = self.parts_dir(export_id)?;
        match fs::remove_dir_all(&directory) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(storage_failed("deleting", &directory, error)),
        }
    }

    fn parts_bytes(&self) -> AppResult<u64> {
        let mut bytes = 0;
        // A finished or cancelled export deletes its parts while this counts; those are simply gone.
        for directory in subdirectories(&self.exports_dir.join(PARTS_DIR))? {
            let entries = match fs::read_dir(&directory) {
                Ok(entries) => entries,
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => return Err(storage_failed("listing", &directory, error)),
            };
            for entry in entries {
                let entry = entry.map_err(|error| storage_failed("listing", &directory, error))?;
                match entry.metadata() {
                    Ok(metadata) if metadata.is_file() => bytes += metadata.len(),
                    Ok(_) => {}
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                    Err(error) => return Err(storage_failed("inspecting", &entry.path(), error)),
                }
            }
        }
        Ok(bytes)
    }

    /// Only files inside the exports folder are deleted.
    fn delete_file(&self, path: &Path) -> AppResult<()> {
        let climbs_out = path.components().any(|component| component == Component::ParentDir);
        if climbs_out || !path.starts_with(&self.exports_dir) {
            return Err(AppError::new(ErrorName::Internal).with_detail(format!("{} is not an export file", path.display())));
        }
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(storage_failed("deleting", path, error)),
        }
    }
}

/// Dropping it without `finish` deletes the partial file.
struct WavPartWriter {
    index: u32,
    path: PathBuf,
    sample_rate: u32,
    samples: u64,
    wav: WavWriter<BufWriter<File>>,
    temp: NamedTempFile,
}

impl ExportWriter for WavPartWriter {
    fn append(&mut self, audio: &Audio) -> AppResult<()> {
        if audio.sample_rate != self.sample_rate {
            return Err(AppError::new(ErrorName::Internal).with_detail(format!("export is {} Hz but a clip is {} Hz", self.sample_rate, audio.sample_rate)));
        }
        for &sample in &audio.samples {
            self.wav.write_sample(to_pcm16(sample)).map_err(|error| storage_failed("writing", self.temp.path(), error))?;
        }
        self.samples += audio.samples.len() as u64;
        Ok(())
    }

    fn finish(self: Box<Self>) -> AppResult<ExportPart> {
        let Self { index, path, sample_rate, samples, wav, temp } = *self;
        wav.finalize().map_err(|error| storage_failed("writing", temp.path(), error))?;
        temp.persist(&path).map_err(|error| storage_failed("renaming", &path, error.error))?;
        Ok(ExportPart { index, path, samples, sample_rate })
    }
}

/// `CREATE_NO_WINDOW`: without it, every ffmpeg run from a Windows GUI app flashes a console window.
#[cfg(windows)]
fn ffmpeg_command(program: &Path) -> Command {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let mut command = Command::new(program);
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

#[cfg(not(windows))]
fn ffmpeg_command(program: &Path) -> Command {
    Command::new(program)
}

fn quiet_args() -> [&'static str; 4] {
    ["-hide_banner", "-nostdin", "-loglevel", "error"]
}

fn mp3_args(target: &ExportTarget) -> Vec<OsString> {
    let mut args: Vec<OsString> = ["-c:a", "libmp3lame", "-b:a", BITRATE, "-ac", MONO, "-id3v2_version", "3"].into_iter().map(OsString::from).collect();
    for (key, value) in [("title", target.title.as_str()), ("artist", target.artist.as_str()), ("album", target.title.as_str()), ("genre", GENRE)] {
        args.push("-metadata".into());
        args.push(format!("{key}={value}").into());
    }
    args.extend(["-f", "mp3"].map(OsString::from));
    args
}

/// Input 1 is the ffmetadata file with the tags and chapter marks.
fn m4b_args() -> [&'static str; 16] {
    ["-map", "0:a", "-map_metadata", "1", "-map_chapters", "1", "-c:a", "aac", "-b:a", BITRATE, "-ac", MONO, "-movflags", "+faststart", "-f", "ipod"]
}

/// The concat demuxer's list: one quoted path per line (a `'` inside is closed, escaped and reopened).
fn concat_list(parts: &[&ExportPart]) -> AppResult<String> {
    let mut list = String::from(CONCAT_HEADER);
    for part in parts {
        let path =
            part.path.to_str().ok_or_else(|| AppError::new(ErrorName::Internal).with_detail(format!("part path is not UTF-8: {}", part.path.display())))?;
        list.push_str(&format!("file '{}'\n", path.replace('\'', r"'\''")));
    }
    Ok(list)
}

fn partial_file(directory: &Path, extension: &str) -> AppResult<NamedTempFile> {
    tempfile::Builder::new()
        .prefix(PARTIAL_PREFIX)
        .suffix(&format!(".{extension}"))
        .tempfile_in(directory)
        .map_err(|error| storage_failed("creating a partial file in", directory, error))
}

fn write_partial(directory: &Path, extension: &str, text: &str) -> AppResult<NamedTempFile> {
    let mut file = partial_file(directory, extension)?;
    file.write_all(text.as_bytes()).map_err(|error| storage_failed("writing", file.path(), error))?;
    Ok(file)
}

fn stderr_tail(log: &mut NamedTempFile) -> String {
    let file = log.as_file_mut();
    let length = file.metadata().map(|metadata| metadata.len()).unwrap_or(0);
    let mut tail = String::new();
    if file.seek(SeekFrom::Start(length.saturating_sub(STDERR_TAIL_BYTES))).is_ok() {
        let mut bytes = Vec::new();
        let _ = file.read_to_end(&mut bytes);
        tail = String::from_utf8_lossy(&bytes).trim().to_string();
    }
    tail
}

fn subdirectories(directory: &Path) -> AppResult<Vec<PathBuf>> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(storage_failed("listing", directory, error)),
    };
    let mut directories = Vec::new();
    for entry in entries {
        let path = entry.map_err(|error| storage_failed("listing", directory, error))?.path();
        if path.is_dir() {
            directories.push(path);
        }
    }
    Ok(directories)
}

/// The ffmpeg detail stays in the log; the client only sees `ExporterUnavailable`.
fn spawn_failed(program: &Path, error: io::Error) -> AppError {
    let name = if error.kind() == io::ErrorKind::NotFound { ErrorName::ExporterUnavailable } else { ErrorName::Internal };
    AppError::new(name).with_detail(format!("cannot run {}", program.display())).with_source(error)
}
