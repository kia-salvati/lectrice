use std::ffi::OsString;
use std::fs;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};

use lectrice_domain::{AppError, AppResult, Audio, AudioExporter, ErrorName, ExportFormat, ExportTarget, ExportWriter, ExportedFile};
use tempfile::NamedTempFile;

use super::ffmetadata::{ChapterMark, Tags, ffmetadata};
use crate::storage::{safe_component, storage_failed};

const EXPORTS_DIR: &str = "exports";
/// Partial files start with this, so a crash's leftovers can be found and removed.
const PARTIAL_PREFIX: &str = ".partial-";
const BITRATE: &str = "64k";
const MONO: &str = "1";
const GENRE: &str = "Audiobook";
/// How much of ffmpeg's error output goes into an error detail.
const STDERR_TAIL_BYTES: u64 = 2000;

/// MP3 (libmp3lame) and M4B (aac, with chapter marks) through the system `ffmpeg`, fed 16-bit mono PCM on stdin.
/// Files go to `<data_dir>/exports/<docId>/<voiceId>.<ext>`, written to a partial file first and renamed when done.
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
    pub fn remove_partial_files(&self) -> AppResult<usize> {
        let mut removed = 0;
        for directory in subdirectories(&self.exports_dir)? {
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
}

impl AudioExporter for FfmpegExporter {
    fn check_available(&self) -> AppResult<()> {
        let status = Command::new(&self.program).arg("-version").stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status();
        match status {
            Ok(status) if status.success() => Ok(()),
            Ok(status) => Err(AppError::new(ErrorName::ExporterUnavailable).with_detail(format!("{} -version exited with {status}", self.program.display()))),
            Err(error) => Err(spawn_failed(&self.program, error)),
        }
    }

    fn begin(&self, target: &ExportTarget, sample_rate: u32) -> AppResult<Box<dyn ExportWriter>> {
        let directory = self.exports_dir.join(target.document_id.as_str());
        let target_path = directory.join(format!("{}.{}", safe_component(target.voice_id.as_str())?, target.format.extension()));
        fs::create_dir_all(&directory).map_err(|error| storage_failed("creating", &directory, error))?;
        let encoded = partial_file(&directory, target.format.extension())?;
        let stderr_log = partial_file(&directory, "log")?;

        let mut command = Command::new(&self.program);
        command.args(quiet_args()).args(pcm_input_args(sample_rate));
        match target.format {
            ExportFormat::Mp3 => command.args(mp3_args(target)),
            ExportFormat::M4b => command.args(aac_args()),
        };
        let stderr = stderr_log.reopen().map_err(|error| storage_failed("opening", stderr_log.path(), error))?;
        let mut child = command
            .arg("-y")
            .arg(encoded.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(stderr)
            .spawn()
            .map_err(|error| spawn_failed(&self.program, error))?;
        let stdin = child.stdin.take();

        Ok(Box::new(FfmpegWriter {
            program: self.program.clone(),
            child: Some(child),
            stdin,
            stderr_log,
            encoded: Some(encoded),
            directory,
            target_path,
            target: target.clone(),
            sample_rate,
            samples_written: 0,
            chapters: Vec::new(),
            buffer: Vec::new(),
        }))
    }
}

struct FfmpegWriter {
    program: PathBuf,
    /// `None` once ffmpeg has exited and been waited for.
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    stderr_log: NamedTempFile,
    /// What the encoder writes: the final MP3, or the M4B's audio before chapters are added. Taken by `finish`.
    encoded: Option<NamedTempFile>,
    directory: PathBuf,
    target_path: PathBuf,
    target: ExportTarget,
    sample_rate: u32,
    samples_written: u64,
    chapters: Vec<ChapterMark>,
    buffer: Vec<u8>,
}

impl ExportWriter for FfmpegWriter {
    fn start_chapter(&mut self, title: &str) -> AppResult<()> {
        self.chapters.push(ChapterMark { title: title.to_string(), start_sample: self.samples_written });
        Ok(())
    }

    fn append(&mut self, audio: &Audio) -> AppResult<()> {
        if audio.sample_rate != self.sample_rate {
            return Err(AppError::new(ErrorName::Internal).with_detail(format!("export is {} Hz but a clip is {} Hz", self.sample_rate, audio.sample_rate)));
        }
        self.buffer.clear();
        self.buffer.extend(audio.samples.iter().flat_map(|&sample| to_pcm16(sample).to_le_bytes()));
        let Some(stdin) = self.stdin.as_mut() else {
            return Err(AppError::new(ErrorName::Internal).with_detail("ffmpeg input is already closed"));
        };
        if let Err(error) = stdin.write_all(&self.buffer) {
            let status = self.wait_for_encoder();
            return Err(self.encoder_failed(status.ok(), "ffmpeg stopped reading audio").with_source(error));
        }
        self.samples_written += audio.samples.len() as u64;
        Ok(())
    }

    fn finish(mut self: Box<Self>) -> AppResult<ExportedFile> {
        let status = self.wait_for_encoder().map_err(|error| AppError::new(ErrorName::Internal).with_detail("waiting for ffmpeg").with_source(error))?;
        if !status.success() {
            return Err(self.encoder_failed(Some(status), "ffmpeg failed to encode"));
        }
        let Some(encoded) = self.encoded.take() else {
            return Err(AppError::new(ErrorName::Internal).with_detail("export was already finished"));
        };
        let finished = match self.target.format {
            ExportFormat::Mp3 => encoded,
            ExportFormat::M4b => self.add_chapters(&encoded)?,
        };
        finished.persist(&self.target_path).map_err(|error| storage_failed("renaming", &self.target_path, error.error))?;
        let size_bytes = fs::metadata(&self.target_path).map_err(|error| storage_failed("inspecting", &self.target_path, error))?.len();
        Ok(ExportedFile { path: self.target_path.clone(), size_bytes })
    }
}

impl FfmpegWriter {
    fn wait_for_encoder(&mut self) -> io::Result<ExitStatus> {
        drop(self.stdin.take());
        let Some(mut child) = self.child.take() else {
            return Err(io::Error::other("ffmpeg was already waited for"));
        };
        child.wait()
    }

    /// Copies the AAC audio into an ipod (M4B) container with the tags and chapter marks.
    fn add_chapters(&mut self, encoded: &NamedTempFile) -> AppResult<NamedTempFile> {
        let tags = Tags { title: &self.target.title, artist: &self.target.artist };
        let metadata = ffmetadata(&tags, &self.chapters, self.samples_written, self.sample_rate);
        let mut metadata_file = partial_file(&self.directory, "txt")?;
        metadata_file.write_all(metadata.as_bytes()).map_err(|error| storage_failed("writing", metadata_file.path(), error))?;
        let output = partial_file(&self.directory, ExportFormat::M4b.extension())?;
        let stderr = self.stderr_log.reopen().map_err(|error| storage_failed("opening", self.stderr_log.path(), error))?;

        let status = Command::new(&self.program)
            .args(quiet_args())
            .arg("-i")
            .arg(encoded.path())
            .args(["-f", "ffmetadata", "-i"])
            .arg(metadata_file.path())
            .args(["-map", "0:a", "-map_metadata", "1", "-map_chapters", "1", "-c", "copy", "-movflags", "+faststart", "-f", "ipod", "-y"])
            .arg(output.path())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(stderr)
            .status()
            .map_err(|error| spawn_failed(&self.program, error))?;
        if !status.success() {
            return Err(self.encoder_failed(Some(status), "ffmpeg failed to add chapters"));
        }
        Ok(output)
    }

    fn encoder_failed(&mut self, status: Option<ExitStatus>, what: &str) -> AppError {
        let status = status.map_or_else(|| "unknown".to_string(), |status| status.to_string());
        AppError::new(ErrorName::Internal).with_detail(format!("{what} (exit {status}): {}", stderr_tail(&mut self.stderr_log)))
    }
}

/// Abandoning an export stops ffmpeg; the partial files are deleted when their handles drop.
impl Drop for FfmpegWriter {
    fn drop(&mut self) {
        drop(self.stdin.take());
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn quiet_args() -> [&'static str; 4] {
    ["-hide_banner", "-nostdin", "-loglevel", "error"]
}

fn pcm_input_args(sample_rate: u32) -> Vec<OsString> {
    ["-f", "s16le", "-ar", &sample_rate.to_string(), "-ac", MONO, "-i", "pipe:0"].into_iter().map(OsString::from).collect()
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

fn aac_args() -> [&'static str; 8] {
    ["-c:a", "aac", "-b:a", BITRATE, "-ac", MONO, "-f", "mp4"]
}

fn to_pcm16(sample: f32) -> i16 {
    (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)).round() as i16
}

fn partial_file(directory: &Path, extension: &str) -> AppResult<NamedTempFile> {
    tempfile::Builder::new()
        .prefix(PARTIAL_PREFIX)
        .suffix(&format!(".{extension}"))
        .tempfile_in(directory)
        .map_err(|error| storage_failed("creating a partial file in", directory, error))
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

fn spawn_failed(program: &Path, error: io::Error) -> AppError {
    let name = if error.kind() == io::ErrorKind::NotFound { ErrorName::ExporterUnavailable } else { ErrorName::Internal };
    AppError::new(name).with_detail(format!("cannot run {}", program.display())).with_source(error)
}
