use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io::{self, BufReader, Cursor, Read};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::SystemTime;

use hound::WavReader;
use lectrice_domain::{AppResult, AudioCache, AudioEviction, AudioKey, AudioRetention, CacheUsage, DocumentCacheUsage, DocumentId, SentenceId, VoiceId};

use super::atomic_file::write_atomically;
use super::path_component::safe_component;
use super::storage_failed;

const AUDIO_DIR: &str = "audio";
const CLIP_EXTENSION: &str = "wav";
const CLEANER_VERSION_PREFIX: &str = "c";

/// Sentence clips at `<cache_dir>/audio/<docId>/c<cleanerVersion>/<engineId>/<voiceId>/<sentenceId>.wav` (spec §6;
/// the desktop's `cache_dir` is its `data_dir`).
///
/// An in-memory index (built by one scan in [`FsAudioCache::open`], then kept up to date by `put` and evictions)
/// answers `contains` and sizes the eviction without touching the disk. Each clip's length comes from its WAV
/// header, so any sample rate works; bytes that aren't a WAV count as no audio for the window (they still count
/// toward the size cap). Clips being read or written are marked in use and never evicted.
pub struct FsAudioCache {
    root: PathBuf,
    max_bytes: u64,
    state: Mutex<CacheState>,
}

/// Document, cleaner version, engine and voice: the clips one listener's window is measured in.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Scope {
    document_id: DocumentId,
    cleaner_version: u32,
    engine_id: String,
    voice_id: VoiceId,
}

impl Scope {
    fn of(key: &AudioKey) -> Self {
        Self { document_id: key.document_id.clone(), cleaner_version: key.cleaner_version, engine_id: key.engine_id.clone(), voice_id: key.voice_id.clone() }
    }

    fn key(&self, sentence_id: u32) -> AudioKey {
        AudioKey {
            document_id: self.document_id.clone(),
            cleaner_version: self.cleaner_version,
            engine_id: self.engine_id.clone(),
            voice_id: self.voice_id.clone(),
            sentence_id: SentenceId(sentence_id),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Clip {
    file_bytes: u64,
    audio_seconds: f64,
    written: SystemTime,
}

/// The length a WAV header announces; `None` when the bytes aren't a readable WAV.
fn wav_seconds(reader: impl Read) -> Option<f64> {
    let wav = WavReader::new(reader).ok()?;
    let sample_rate = wav.spec().sample_rate;
    (sample_rate > 0).then(|| f64::from(wav.duration()) / f64::from(sample_rate))
}

#[derive(Default)]
struct CacheState {
    clips: HashMap<Scope, BTreeMap<u32, Clip>>,
    total_bytes: u64,
    /// Clips being read or written, with how many callers use each.
    in_use: HashMap<AudioKey, usize>,
    /// Clips of a removed document that were in use; deleted when the last user lets go.
    doomed: HashSet<AudioKey>,
}

impl CacheState {
    fn insert(&mut self, key: &AudioKey, clip: Clip) {
        let previous = self.clips.entry(Scope::of(key)).or_default().insert(key.sentence_id.0, clip);
        self.total_bytes = self.total_bytes - previous.map_or(0, |old| old.file_bytes) + clip.file_bytes;
    }

    fn remove(&mut self, scope: &Scope, sentence_id: u32) {
        let Some(clips) = self.clips.get_mut(scope) else {
            return;
        };
        if let Some(clip) = clips.remove(&sentence_id) {
            self.total_bytes -= clip.file_bytes;
        }
        if clips.is_empty() {
            self.clips.remove(scope);
        }
    }

    fn is_in_use(&self, key: &AudioKey) -> bool {
        self.in_use.contains_key(key)
    }
}

impl FsAudioCache {
    /// Scans the existing clips under `<cache_dir>/audio` once. `max_bytes` is the global cap (`audio.cache_max_mb`).
    /// `cache_dir` may be a folder the system clears on its own (Android's `cacheDir`): a missing, empty or unreadable
    /// clip is a miss, and [`AudioRetention::refresh`] re-reads the folder.
    pub fn open(cache_dir: &Path, max_bytes: u64) -> AppResult<Self> {
        let root = cache_dir.join(AUDIO_DIR);
        let mut state = CacheState::default();
        for (key, clip) in scan(&root)? {
            state.insert(&key, clip);
        }
        Ok(Self { root, max_bytes, state: Mutex::new(state) })
    }

    /// No caller panics while holding the lock in a way that leaves the index half-updated.
    fn lock(&self) -> MutexGuard<'_, CacheState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn clip_path(&self, key: &AudioKey) -> AppResult<PathBuf> {
        let engine_id = safe_component(&key.engine_id)?;
        let voice_id = safe_component(key.voice_id.as_str())?;
        Ok(self
            .root
            .join(key.document_id.as_str())
            .join(format!("{CLEANER_VERSION_PREFIX}{}", key.cleaner_version))
            .join(engine_id)
            .join(voice_id)
            .join(format!("{}.{CLIP_EXTENSION}", key.sentence_id.0)))
    }

    fn mark_in_use(&self, key: &AudioKey) -> InUse<'_> {
        *self.lock().in_use.entry(key.clone()).or_default() += 1;
        InUse { cache: self, key: key.clone() }
    }

    /// Deletes one clip file and drops it from the index. A file that is already gone counts as deleted.
    fn delete(&self, state: &mut CacheState, scope: &Scope, sentence_id: u32) -> AppResult<u64> {
        let key = scope.key(sentence_id);
        let path = self.clip_path(&key)?;
        let freed = state.clips.get(scope).and_then(|clips| clips.get(&sentence_id)).map_or(0, |clip| clip.file_bytes);
        remove_if_present(&path)?;
        state.remove(scope, sentence_id);
        Ok(freed)
    }

    /// Deletes every clip of the scopes `matches` picks, except the ones in use: those are `doomed` when `defer` is set.
    fn delete_scopes(&self, matches: impl Fn(&Scope) -> bool, defer: bool) -> AppResult<AudioEviction> {
        let mut state = self.lock();
        let victims: Vec<(Scope, u32)> =
            state.clips.iter().filter(|(scope, _)| matches(scope)).flat_map(|(scope, clips)| clips.keys().map(move |&id| (scope.clone(), id))).collect();
        let mut eviction = AudioEviction::default();
        for (scope, sentence_id) in victims {
            let key = scope.key(sentence_id);
            if state.is_in_use(&key) {
                if defer {
                    state.doomed.insert(key);
                }
                continue;
            }
            eviction.freed_bytes += self.delete(&mut state, &scope, sentence_id)?;
            eviction.cap_clips += 1;
        }
        eviction.cache_bytes = state.total_bytes;
        Ok(eviction)
    }

    /// A clip that turned out empty or unreadable is forgotten and deleted, unless a `put` replaced it meanwhile.
    fn forget_broken(&self, key: &AudioKey, path: &Path) {
        let mut state = self.lock();
        let replaced = fs::metadata(path).is_ok_and(|metadata| metadata.len() > 0);
        if !replaced {
            let _ = fs::remove_file(path);
            state.remove(&Scope::of(key), key.sentence_id.0);
        }
    }
}

fn remove_if_present(path: &Path) -> AppResult<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(storage_failed("deleting", path, error)),
    }
}

/// Unmarks a clip as in use when dropped; the last user of a doomed clip deletes it.
struct InUse<'a> {
    cache: &'a FsAudioCache,
    key: AudioKey,
}

impl Drop for InUse<'_> {
    fn drop(&mut self) {
        let mut state = self.cache.lock();
        let Some(count) = state.in_use.get_mut(&self.key) else { return };
        *count -= 1;
        if *count > 0 {
            return;
        }
        state.in_use.remove(&self.key);
        if state.doomed.remove(&self.key) {
            // A failed delete leaves the file to the next clear, refresh or removal.
            let _ = self.cache.delete(&mut state, &Scope::of(&self.key), self.key.sentence_id.0);
        }
    }
}

impl AudioCache for FsAudioCache {
    fn get(&self, key: &AudioKey) -> AppResult<Option<Vec<u8>>> {
        let path = self.clip_path(key)?;
        let _in_use = self.mark_in_use(key);
        match fs::read(&path) {
            Ok(bytes) if !bytes.is_empty() => Ok(Some(bytes)),
            Ok(_) => {
                self.forget_broken(key, &path);
                Ok(None)
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                // A `put` may have written the clip since the read failed; only forget it if it's still gone.
                let mut state = self.lock();
                if !path.exists() {
                    state.remove(&Scope::of(key), key.sentence_id.0);
                }
                Ok(None)
            }
            // The clip can be made again, so an unreadable file is a miss rather than an error.
            Err(_) => {
                self.forget_broken(key, &path);
                Ok(None)
            }
        }
    }

    fn put(&self, key: &AudioKey, bytes: &[u8]) -> AppResult<()> {
        let path = self.clip_path(key)?;
        let in_use = self.mark_in_use(key);
        write_creating_folders(&path, bytes)?;
        let audio_seconds = wav_seconds(Cursor::new(bytes)).unwrap_or_default();
        self.lock().insert(key, Clip { file_bytes: bytes.len() as u64, audio_seconds, written: SystemTime::now() });
        drop(in_use);
        Ok(())
    }
}

/// Creates the clip's folders, and once more if the system deleted them between that and the write.
fn write_creating_folders(path: &Path, bytes: &[u8]) -> AppResult<()> {
    let directory = path.parent().unwrap_or(path);
    let mut retried = false;
    loop {
        fs::create_dir_all(directory).map_err(|error| storage_failed("creating", directory, error))?;
        match write_atomically(path, bytes) {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound && !retried => retried = true,
            Err(error) => return Err(storage_failed("writing", path, error)),
        }
    }
}

impl AudioRetention for FsAudioCache {
    fn contains(&self, key: &AudioKey) -> AppResult<bool> {
        self.clip_path(key)?;
        Ok(self.lock().clips.get(&Scope::of(key)).is_some_and(|clips| clips.contains_key(&key.sentence_id.0)))
    }

    fn retain_around(&self, position: &AudioKey, keep_seconds: f64) -> AppResult<AudioEviction> {
        let scope = Scope::of(position);
        let mut state = self.lock();
        let mut eviction = AudioEviction::default();

        let window = state.clips.get(&scope).map(|clips| window_victims(clips, position.sentence_id.0, keep_seconds, |id| state.is_in_use(&scope.key(id))));
        for sentence_id in window.unwrap_or_default() {
            eviction.freed_bytes += self.delete(&mut state, &scope, sentence_id)?;
            eviction.window_clips += 1;
        }

        if state.total_bytes > self.max_bytes {
            for (victim_scope, sentence_id) in cap_victims(&state, &scope, self.max_bytes) {
                eviction.freed_bytes += self.delete(&mut state, &victim_scope, sentence_id)?;
                eviction.cap_clips += 1;
            }
        }

        eviction.kept_seconds = state.clips.get(&scope).map_or(0.0, |clips| clips.values().map(|clip| clip.audio_seconds).sum());
        eviction.cache_bytes = state.total_bytes;
        Ok(eviction)
    }

    fn usage(&self) -> AppResult<CacheUsage> {
        let state = self.lock();
        let mut documents: HashMap<&DocumentId, DocumentCacheUsage> = HashMap::new();
        for (scope, clips) in &state.clips {
            let usage =
                documents.entry(&scope.document_id).or_insert_with(|| DocumentCacheUsage { document_id: scope.document_id.clone(), bytes: 0, clips: 0 });
            usage.bytes += clips.values().map(|clip| clip.file_bytes).sum::<u64>();
            usage.clips += clips.len();
        }
        let clips = documents.values().map(|usage| usage.clips).sum();
        Ok(CacheUsage { bytes: state.total_bytes, clips, cap_bytes: self.max_bytes, documents: documents.into_values().collect() })
    }

    fn clear(&self) -> AppResult<AudioEviction> {
        self.delete_scopes(|_| true, false)
    }

    /// Also deletes the document's folder when nothing in it is in use, so files the index never knew go too.
    fn remove_document(&self, id: &DocumentId) -> AppResult<AudioEviction> {
        let eviction = self.delete_scopes(|scope| scope.document_id == *id, true)?;
        let state = self.lock();
        if !state.in_use.keys().any(|key| key.document_id == *id) {
            let directory = self.root.join(id.as_str());
            match fs::remove_dir_all(&directory) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(storage_failed("deleting", &directory, error)),
            }
        }
        Ok(eviction)
    }

    fn refresh(&self) -> AppResult<()> {
        let found = scan(&self.root)?;
        let mut state = self.lock();
        state.clips.clear();
        state.total_bytes = 0;
        for (key, clip) in found {
            state.insert(&key, clip);
        }
        Ok(())
    }
}

/// Which clips of one scope to delete so at most `keep_seconds` of audio stay: the farthest behind
/// `position` first, then the farthest ahead. The clip at `position` and clips in use are never chosen.
/// A NaN `keep_seconds` (a caller's bug) deletes nothing rather than the whole scope.
fn window_victims(clips: &BTreeMap<u32, Clip>, position: u32, keep_seconds: f64, in_use: impl Fn(u32) -> bool) -> Vec<u32> {
    if keep_seconds.is_nan() {
        return Vec::new();
    }
    let mut total: f64 = clips.values().map(|clip| clip.audio_seconds).sum();
    let behind = clips.range(..position);
    let ahead = clips.range(position.saturating_add(1)..).rev();
    let mut victims = Vec::new();
    for (&id, clip) in behind.chain(ahead) {
        if total <= keep_seconds {
            break;
        }
        if in_use(id) {
            continue;
        }
        total -= clip.audio_seconds;
        victims.push(id);
    }
    victims
}

/// Oldest clips first, outside the listener's own scope (already trimmed to its window) and not in use,
/// until the cache fits in `max_bytes`.
fn cap_victims(state: &CacheState, protected: &Scope, max_bytes: u64) -> Vec<(Scope, u32)> {
    let mut candidates: Vec<(SystemTime, &Scope, u32, u64)> = state
        .clips
        .iter()
        .filter(|(scope, _)| *scope != protected)
        .flat_map(|(scope, clips)| clips.iter().map(move |(&id, clip)| (clip.written, scope, id, clip.file_bytes)))
        .filter(|(_, scope, id, _)| !state.is_in_use(&scope.key(*id)))
        .collect();
    candidates.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.2.cmp(&b.2)));
    let mut total = state.total_bytes;
    let mut victims = Vec::new();
    for (_, scope, id, bytes) in candidates {
        if total <= max_bytes {
            break;
        }
        total -= bytes;
        victims.push((scope.clone(), id));
    }
    victims
}

/// Every clip under `root` with its size and write time. Anything that doesn't fit the layout (temp files,
/// foreign folders) is ignored.
fn scan(root: &Path) -> AppResult<Vec<(AudioKey, Clip)>> {
    let mut found = Vec::new();
    for (document_dir, document_name) in subdirectories(root)? {
        let Ok(document_id) = DocumentId::parse(&document_name) else { continue };
        for (version_dir, version_name) in subdirectories(&document_dir)? {
            let Some(cleaner_version) = version_name.strip_prefix(CLEANER_VERSION_PREFIX).and_then(|digits| digits.parse().ok()) else { continue };
            for (engine_dir, engine_id) in subdirectories(&version_dir)? {
                for (voice_dir, voice_id) in subdirectories(&engine_dir)? {
                    let scope = Scope { document_id: document_id.clone(), cleaner_version, engine_id: engine_id.clone(), voice_id: VoiceId::new(voice_id) };
                    scan_clips(&voice_dir, &scope, &mut found)?;
                }
            }
        }
    }
    Ok(found)
}

fn scan_clips(directory: &Path, scope: &Scope, found: &mut Vec<(AudioKey, Clip)>) -> AppResult<()> {
    for entry in read_dir(directory)? {
        let path = entry.path();
        let is_clip = path.extension().is_some_and(|extension| extension == CLIP_EXTENSION);
        let Some(sentence_id) = path.file_stem().and_then(|stem| stem.to_str()).and_then(|stem| stem.parse().ok()).filter(|_| is_clip) else { continue };
        let metadata = match entry.metadata() {
            Ok(metadata) => metadata,
            // The system cleared it between the listing and now.
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(storage_failed("scanning", &path, error)),
        };
        if !metadata.is_file() {
            continue;
        }
        let written = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        // Only the header is read. A clip that can't be opened now counts as no audio, like a non-WAV one.
        let audio_seconds = fs::File::open(&path).ok().and_then(|file| wav_seconds(BufReader::new(file))).unwrap_or_default();
        found.push((scope.key(sentence_id), Clip { file_bytes: metadata.len(), audio_seconds, written }));
    }
    Ok(())
}

/// Plain-named subdirectories (see [`safe_component`]); a missing directory has none.
fn subdirectories(directory: &Path) -> AppResult<Vec<(PathBuf, String)>> {
    let mut found = Vec::new();
    for entry in read_dir(directory)? {
        let path = entry.path();
        let Some(name) = entry.file_name().to_str().map(str::to_string) else { continue };
        if path.is_dir() && safe_component(&name).is_ok() {
            found.push((path, name));
        }
    }
    Ok(found)
}

fn read_dir(directory: &Path) -> AppResult<Vec<fs::DirEntry>> {
    match fs::read_dir(directory) {
        Ok(entries) => entries.collect::<io::Result<Vec<_>>>().map_err(|error| storage_failed("scanning", directory, error)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(storage_failed("scanning", directory, error)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `seconds` of audio per clip, ids `0..count`.
    fn clips(count: u32, seconds: f64) -> BTreeMap<u32, Clip> {
        let clip = Clip { file_bytes: 1, audio_seconds: seconds, written: SystemTime::UNIX_EPOCH };
        (0..count).map(|id| (id, clip)).collect()
    }

    #[test]
    fn a_window_that_fits_deletes_nothing() {
        assert!(window_victims(&clips(10, 1.0), 5, 10.0, |_| false).is_empty());
    }

    #[test]
    fn the_farthest_behind_go_first() {
        assert_eq!(window_victims(&clips(10, 1.0), 5, 7.0, |_| false), [0, 1, 2]);
    }

    #[test]
    fn when_behind_is_not_enough_the_farthest_ahead_go_next() {
        assert_eq!(window_victims(&clips(10, 1.0), 3, 4.0, |_| false), [0, 1, 2, 9, 8, 7]);
    }

    #[test]
    fn the_position_and_clips_in_use_are_never_chosen() {
        assert_eq!(window_victims(&clips(4, 1.0), 2, 0.0, |id| id == 0), [1, 3]);
    }

    #[test]
    fn a_nan_window_deletes_nothing() {
        assert!(window_victims(&clips(3, 1.0), 1, f64::NAN, |_| false).is_empty());
    }

    #[test]
    fn the_length_comes_from_the_wav_header_at_any_sample_rate() {
        use lectrice_domain::{Audio, AudioEncoder};
        for sample_rate in [22_050, 24_000, 44_100] {
            let audio = Audio { samples: vec![0.0; sample_rate as usize * 2], sample_rate };
            let bytes = crate::WavEncoder.encode(&audio).unwrap();
            assert_eq!(wav_seconds(Cursor::new(bytes)), Some(2.0));
        }
        assert_eq!(wav_seconds(Cursor::new(b"not a wav")), None);
    }
}
