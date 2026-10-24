use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use lectrice_domain::{Audio, AudioCache, AudioEncoder, AudioKey, AudioRetention, DocumentId, ErrorName, SentenceId, VoiceId};
use lectrice_infra::{FsAudioCache, WavEncoder};
use tempfile::TempDir;

const NO_CAP: u64 = u64::MAX;
const SAMPLE_RATE: u32 = 24_000;
/// One second of 16-bit mono 24 kHz audio plus its 44-byte WAV header.
const ONE_SECOND_CLIP: usize = 44 + 2 * SAMPLE_RATE as usize;

/// A WAV clip of `seconds` at `sample_rate`, every sample set to `level`.
fn wav(seconds: f64, sample_rate: u32, level: f32) -> Vec<u8> {
    let samples = vec![level; (seconds * f64::from(sample_rate)) as usize];
    WavEncoder.encode(&Audio { samples, sample_rate }).unwrap()
}

fn one_second_clip(level: f32) -> Vec<u8> {
    let clip = wav(1.0, SAMPLE_RATE, level);
    assert_eq!(clip.len(), ONE_SECOND_CLIP);
    clip
}

fn key(voice: &str, engine: &str) -> AudioKey {
    AudioKey {
        document_id: DocumentId::parse(&format!("{:064x}", 7)).unwrap(),
        cleaner_version: 3,
        engine_id: engine.to_string(),
        voice_id: VoiceId::new(voice),
        sentence_id: SentenceId(42),
    }
}

fn clip_key(document: u8, voice: &str, sentence: u32) -> AudioKey {
    AudioKey {
        document_id: DocumentId::parse(&format!("{document:064x}")).unwrap(),
        cleaner_version: 1,
        engine_id: "kokoro-v1.0".into(),
        voice_id: VoiceId::new(voice),
        sentence_id: SentenceId(sentence),
    }
}

fn clip_path(root: &Path, key: &AudioKey) -> PathBuf {
    root.join("audio")
        .join(key.document_id.as_str())
        .join(format!("c{}", key.cleaner_version))
        .join(&key.engine_id)
        .join(key.voice_id.as_str())
        .join(format!("{}.wav", key.sentence_id.0))
}

/// Puts one-second clips for `sentences` and returns their keys.
fn put_clips(cache: &FsAudioCache, document: u8, voice: &str, sentences: impl IntoIterator<Item = u32>) -> Vec<AudioKey> {
    let clip = one_second_clip(0.25);
    sentences
        .into_iter()
        .map(|sentence| {
            let key = clip_key(document, voice, sentence);
            cache.put(&key, &clip).unwrap();
            key
        })
        .collect()
}

fn cached_sentences(cache: &FsAudioCache, document: u8, voice: &str, sentences: impl IntoIterator<Item = u32>) -> Vec<u32> {
    sentences.into_iter().filter(|&sentence| cache.contains(&clip_key(document, voice, sentence)).unwrap()).collect()
}

#[test]
fn a_put_clip_is_read_back_from_the_spec_path() {
    let dir = TempDir::new().unwrap();
    let cache = FsAudioCache::open(dir.path(), NO_CAP).unwrap();
    let key = key("af_heart", "kokoro-v1.0");

    assert_eq!(cache.get(&key).unwrap(), None);
    assert!(!cache.contains(&key).unwrap());
    cache.put(&key, b"RIFF clip").unwrap();

    assert_eq!(cache.get(&key).unwrap().as_deref(), Some(&b"RIFF clip"[..]));
    assert!(cache.contains(&key).unwrap());
    let expected = dir.path().join("audio").join(key.document_id.as_str()).join("c3/kokoro-v1.0/af_heart/42.wav");
    assert!(expected.is_file());
}

#[test]
fn a_second_put_replaces_the_clip() {
    let dir = TempDir::new().unwrap();
    let cache = FsAudioCache::open(dir.path(), NO_CAP).unwrap();
    let key = key("af_heart", "kokoro-v1.0");

    cache.put(&key, b"old").unwrap();
    cache.put(&key, b"new").unwrap();

    assert_eq!(cache.get(&key).unwrap().as_deref(), Some(&b"new"[..]));
}

#[test]
fn path_like_ids_are_refused() {
    let dir = TempDir::new().unwrap();
    let cache = FsAudioCache::open(dir.path(), NO_CAP).unwrap();

    for bad in [key("../x", "kokoro"), key("af_heart", ".."), key("", "kokoro"), key("a/b", "kokoro")] {
        assert_eq!(cache.put(&bad, b"x").unwrap_err().primary(), ErrorName::StorageFailed);
        assert_eq!(cache.get(&bad).unwrap_err().primary(), ErrorName::StorageFailed);
        assert_eq!(cache.contains(&bad).unwrap_err().primary(), ErrorName::StorageFailed);
    }
}

#[test]
fn reopening_finds_the_clips_already_on_disk_and_ignores_strays() {
    let dir = TempDir::new().unwrap();
    put_clips(&FsAudioCache::open(dir.path(), NO_CAP).unwrap(), 1, "af_heart", 0..3);
    let voice_dir = clip_path(dir.path(), &clip_key(1, "af_heart", 0)).parent().unwrap().to_path_buf();
    std::fs::write(voice_dir.join(".tmpAbC123"), b"half").unwrap();
    std::fs::create_dir_all(dir.path().join("audio/not-a-document/c1/e/v")).unwrap();

    let reopened = FsAudioCache::open(dir.path(), NO_CAP).unwrap();

    assert_eq!(cached_sentences(&reopened, 1, "af_heart", 0..5), [0, 1, 2]);
    let eviction = reopened.retain_around(&clip_key(1, "af_heart", 0), 10.0).unwrap();
    assert!((eviction.kept_seconds - 3.0).abs() < 1e-9);
    assert_eq!(eviction.cache_bytes, 3 * ONE_SECOND_CLIP as u64);
}

#[test]
fn the_window_deletes_the_farthest_behind_first() {
    let dir = TempDir::new().unwrap();
    let cache = FsAudioCache::open(dir.path(), NO_CAP).unwrap();
    let keys = put_clips(&cache, 1, "af_heart", 0..10);

    let eviction = cache.retain_around(&clip_key(1, "af_heart", 5), 7.0).unwrap();

    assert_eq!((eviction.window_clips, eviction.cap_clips), (3, 0));
    assert_eq!(eviction.freed_bytes, 3 * ONE_SECOND_CLIP as u64);
    assert!((eviction.kept_seconds - 7.0).abs() < 1e-9);
    assert_eq!(cached_sentences(&cache, 1, "af_heart", 0..10), [3, 4, 5, 6, 7, 8, 9]);
    assert!(!clip_path(dir.path(), &keys[2]).exists());
    assert!(clip_path(dir.path(), &keys[3]).exists());
}

#[test]
fn after_everything_behind_the_farthest_ahead_go() {
    let dir = TempDir::new().unwrap();
    let cache = FsAudioCache::open(dir.path(), NO_CAP).unwrap();
    put_clips(&cache, 1, "af_heart", 0..10);

    cache.retain_around(&clip_key(1, "af_heart", 2), 4.0).unwrap();

    assert_eq!(cached_sentences(&cache, 1, "af_heart", 0..10), [2, 3, 4, 5]);
}

#[test]
fn the_window_is_per_document_and_voice() {
    let dir = TempDir::new().unwrap();
    let cache = FsAudioCache::open(dir.path(), NO_CAP).unwrap();
    put_clips(&cache, 1, "af_heart", 0..5);
    put_clips(&cache, 1, "bm_lewis", 0..5);
    put_clips(&cache, 2, "af_heart", 0..5);

    cache.retain_around(&clip_key(1, "af_heart", 4), 1.0).unwrap();

    assert_eq!(cached_sentences(&cache, 1, "af_heart", 0..5), [4]);
    assert_eq!(cached_sentences(&cache, 1, "bm_lewis", 0..5), [0, 1, 2, 3, 4]);
    assert_eq!(cached_sentences(&cache, 2, "af_heart", 0..5), [0, 1, 2, 3, 4]);
}

#[test]
fn the_global_cap_deletes_the_oldest_other_clips_first() {
    let dir = TempDir::new().unwrap();
    let cache = FsAudioCache::open(dir.path(), 9 * ONE_SECOND_CLIP as u64).unwrap();
    put_clips(&cache, 1, "af_heart", 0..5);
    put_clips(&cache, 2, "af_heart", 0..5);
    put_clips(&cache, 3, "af_heart", 0..3);

    let eviction = cache.retain_around(&clip_key(3, "af_heart", 0), 600.0).unwrap();

    assert_eq!((eviction.window_clips, eviction.cap_clips), (0, 4));
    assert_eq!(eviction.cache_bytes, 9 * ONE_SECOND_CLIP as u64);
    assert_eq!(cached_sentences(&cache, 1, "af_heart", 0..5), [4]);
    assert_eq!(cached_sentences(&cache, 2, "af_heart", 0..5), [0, 1, 2, 3, 4]);
    assert_eq!(cached_sentences(&cache, 3, "af_heart", 0..3), [0, 1, 2]);
}

#[test]
fn the_window_runs_before_the_cap() {
    let dir = TempDir::new().unwrap();
    let cache = FsAudioCache::open(dir.path(), 6 * ONE_SECOND_CLIP as u64).unwrap();
    put_clips(&cache, 1, "af_heart", 0..4);
    put_clips(&cache, 2, "af_heart", 0..6);

    let eviction = cache.retain_around(&clip_key(2, "af_heart", 3), 3.0).unwrap();

    assert_eq!((eviction.window_clips, eviction.cap_clips), (3, 1));
    assert_eq!(cached_sentences(&cache, 2, "af_heart", 0..6), [3, 4, 5]);
    assert_eq!(cached_sentences(&cache, 1, "af_heart", 0..4), [1, 2, 3]);
}

#[test]
fn eviction_never_breaks_concurrent_reads_and_writes() {
    let dir = TempDir::new().unwrap();
    let cache = Arc::new(FsAudioCache::open(dir.path(), 20 * ONE_SECOND_CLIP as u64).unwrap());
    let first = one_second_clip(0.25);
    let clip = one_second_clip(0.5);
    put_clips(&cache, 1, "af_heart", 0..40);
    let done = Arc::new(AtomicBool::new(false));

    let readers: Vec<_> = (0..4)
        .map(|_| {
            let (cache, done, first, clip) = (cache.clone(), done.clone(), first.clone(), clip.clone());
            std::thread::spawn(move || {
                while !done.load(Ordering::SeqCst) {
                    for sentence in 0..40 {
                        if let Some(bytes) = cache.get(&clip_key(1, "af_heart", sentence)).unwrap() {
                            assert!(bytes == first || bytes == clip);
                        }
                    }
                }
            })
        })
        .collect();
    let writer = {
        let (cache, clip) = (cache.clone(), clip.clone());
        std::thread::spawn(move || {
            for round in 0..5 {
                for sentence in 0..40 {
                    cache.put(&clip_key(2 + round % 2, "af_heart", sentence), &clip).unwrap();
                }
            }
        })
    };
    for step in 0..200 {
        cache.retain_around(&clip_key(1, "af_heart", step % 40), 5.0).unwrap();
    }
    writer.join().unwrap();
    done.store(true, Ordering::SeqCst);
    for reader in readers {
        reader.join().unwrap();
    }

    let eviction = cache.retain_around(&clip_key(1, "af_heart", 20), 5.0).unwrap();
    assert!(eviction.cache_bytes <= 20 * ONE_SECOND_CLIP as u64);
    let reopened = FsAudioCache::open(dir.path(), NO_CAP).unwrap();
    assert_eq!(reopened.retain_around(&clip_key(1, "af_heart", 20), 1e9).unwrap().cache_bytes, eviction.cache_bytes);
}

/// Review L6: the window is measured in seconds from each clip's header, not from a fixed 24 kHz byte rate.
#[test]
fn the_window_measures_clips_of_any_sample_rate_in_seconds() {
    let dir = TempDir::new().unwrap();
    let cache = FsAudioCache::open(dir.path(), NO_CAP).unwrap();
    for sentence in 0..4 {
        cache.put(&clip_key(1, "af_heart", sentence), &wav(1.0, 44_100, 0.1)).unwrap();
    }

    let eviction = cache.retain_around(&clip_key(1, "af_heart", 3), 2.0).unwrap();

    assert_eq!(cached_sentences(&cache, 1, "af_heart", 0..4), [2, 3]);
    assert!((eviction.kept_seconds - 2.0).abs() < 1e-9);
    let reopened = FsAudioCache::open(dir.path(), NO_CAP).unwrap();
    assert!((reopened.retain_around(&clip_key(1, "af_heart", 3), 60.0).unwrap().kept_seconds - 2.0).abs() < 1e-9);
}

/// Review L2: a `get` that misses while a `put` of the same clip lands must not drop the new clip from the index.
#[test]
fn a_miss_racing_a_put_keeps_the_new_clip() {
    let dir = TempDir::new().unwrap();
    let cache = Arc::new(FsAudioCache::open(dir.path(), NO_CAP).unwrap());
    let clip = one_second_clip(0.25);
    for sentence in 0..300 {
        let key = clip_key(1, "af_heart", sentence);
        let writer = {
            let (cache, key, clip) = (cache.clone(), key.clone(), clip.clone());
            std::thread::spawn(move || cache.put(&key, &clip).unwrap())
        };
        cache.get(&key).unwrap();
        writer.join().unwrap();
        assert!(cache.contains(&key).unwrap(), "sentence {sentence} fell out of the index");
    }
    assert!((cache.retain_around(&clip_key(1, "af_heart", 0), 1e9).unwrap().kept_seconds - 300.0).abs() < 1e-6);
}

/// C29: the system may delete clips at any time; a vanished clip is a miss and can be put again.
#[test]
fn clips_deleted_behind_the_caches_back_are_misses_and_refresh_forgets_them() {
    let dir = TempDir::new().unwrap();
    let cache = FsAudioCache::open(dir.path(), NO_CAP).unwrap();
    let keys = put_clips(&cache, 1, "af_heart", 0..3);
    std::fs::remove_dir_all(dir.path().join("audio")).unwrap();

    assert_eq!(cache.get(&keys[0]).unwrap(), None);
    assert_eq!(cache.usage().unwrap().clips, 2, "the index still has the clips nobody asked for");
    cache.refresh().unwrap();
    assert_eq!((cache.usage().unwrap().clips, cache.usage().unwrap().bytes), (0, 0));

    cache.put(&keys[1], &one_second_clip(0.5)).unwrap();
    assert!(cache.get(&keys[1]).unwrap().is_some(), "folders the system removed are made again");
}

/// C29: an empty clip (the system cleared it halfway) is never returned as audio.
#[test]
fn an_empty_clip_is_a_miss_and_is_deleted() {
    let dir = TempDir::new().unwrap();
    let cache = FsAudioCache::open(dir.path(), NO_CAP).unwrap();
    let key = put_clips(&cache, 1, "af_heart", [0]).remove(0);
    std::fs::write(clip_path(dir.path(), &key), b"").unwrap();

    assert_eq!(cache.get(&key).unwrap(), None);
    assert!(!cache.contains(&key).unwrap());
    assert!(!clip_path(dir.path(), &key).exists());
}

/// C15: usage per document, and a clear deletes every clip.
#[test]
fn usage_is_counted_per_document_and_clear_empties_the_cache() {
    let dir = TempDir::new().unwrap();
    let cache = FsAudioCache::open(dir.path(), 5_000_000).unwrap();
    put_clips(&cache, 1, "af_heart", 0..2);
    put_clips(&cache, 2, "bm_lewis", [0]);

    let usage = cache.usage().unwrap();

    assert_eq!((usage.bytes, usage.clips, usage.cap_bytes), (3 * ONE_SECOND_CLIP as u64, 3, 5_000_000));
    let mut per_document: Vec<(String, u64, usize)> = usage.documents.iter().map(|usage| (usage.document_id.to_string(), usage.bytes, usage.clips)).collect();
    per_document.sort();
    assert_eq!(per_document, [(format!("{:064x}", 1), 2 * ONE_SECOND_CLIP as u64, 2), (format!("{:064x}", 2), ONE_SECOND_CLIP as u64, 1)]);

    let cleared = cache.clear().unwrap();

    assert_eq!((cleared.evicted_clips(), cleared.freed_bytes, cleared.cache_bytes), (3, 3 * ONE_SECOND_CLIP as u64, 0));
    assert!(cached_sentences(&cache, 1, "af_heart", 0..2).is_empty());
}

/// C28: removing a document's audio deletes every cleaner version, engine and voice of it, and only of it.
#[test]
fn removing_a_documents_audio_deletes_all_of_it_and_nothing_else() {
    let dir = TempDir::new().unwrap();
    let cache = FsAudioCache::open(dir.path(), NO_CAP).unwrap();
    put_clips(&cache, 1, "af_heart", 0..2);
    put_clips(&cache, 1, "bm_lewis", [0]);
    let older = AudioKey { cleaner_version: 9, ..clip_key(1, "af_heart", 5) };
    cache.put(&older, &one_second_clip(0.1)).unwrap();
    put_clips(&cache, 2, "af_heart", [0]);
    let document = DocumentId::parse(&format!("{:064x}", 1)).unwrap();

    let eviction = cache.remove_document(&document).unwrap();

    assert_eq!(eviction.evicted_clips(), 4);
    assert!(!dir.path().join("audio").join(document.as_str()).exists());
    assert_eq!(cached_sentences(&cache, 2, "af_heart", [0]), [0]);
    assert_eq!(cache.usage().unwrap().clips, 1);
}

/// C29: the cache can live in its own folder, apart from the library.
#[test]
fn the_cache_root_is_its_own_folder() {
    let dir = TempDir::new().unwrap();
    let cache_dir = dir.path().join("cache");
    let cache = FsAudioCache::open(&cache_dir, NO_CAP).unwrap();
    let key = put_clips(&cache, 1, "af_heart", [0]).remove(0);

    assert!(clip_path(&cache_dir, &key).is_file());
    assert_eq!(FsAudioCache::open(&cache_dir, NO_CAP).unwrap().usage().unwrap().clips, 1);
}
