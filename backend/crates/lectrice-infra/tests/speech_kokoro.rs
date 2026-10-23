use std::path::{Path, PathBuf};

use lectrice_domain::{ErrorName, SpeechEngine, SpeechRequest, VoiceId};
use lectrice_infra::{KokoroSettings, KokoroSpeechEngine};

const NUM_THREADS: u32 = 4;
const CPU_PROVIDER: &str = "cpu";
const KOKORO_SAMPLE_RATE: u32 = 24_000;
const BUNDLE_FILES_WITHOUT_LEXICON: [&str; 7] =
    ["model.onnx", "voices.bin", "tokens.txt", "espeak-ng-data/phontab", "espeak-ng-data/phonindex", "espeak-ng-data/phondata", "espeak-ng-data/intonations"];

fn bundled_model_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../models/kokoro-multi-lang-v1_0")
}

fn settings(model_dir: PathBuf) -> KokoroSettings {
    KokoroSettings { model_dir, num_threads: NUM_THREADS, provider: CPU_PROVIDER.into() }
}

fn load(model_dir: PathBuf) -> KokoroSpeechEngine {
    KokoroSpeechEngine::load(&settings(model_dir)).unwrap()
}

fn load_error(model_dir: PathBuf) -> ErrorName {
    match KokoroSpeechEngine::load(&settings(model_dir)) {
        Ok(_) => panic!("the engine loaded from a broken model dir"),
        Err(error) => error.primary(),
    }
}

#[test]
fn missing_model_dir_leaves_the_engine_unavailable() {
    let dir = tempfile::tempdir().unwrap();

    assert_eq!(load_error(dir.path().join("missing")), ErrorName::SpeechEngineUnavailable);
}

/// Without a lexicon sherpa-onnx would call exit(); reaching the assertion proves we never get that far.
#[test]
fn bundle_without_lexicon_is_rejected_before_sherpa_onnx_sees_it() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("espeak-ng-data")).unwrap();
    for file in BUNDLE_FILES_WITHOUT_LEXICON {
        std::fs::write(dir.path().join(file), b"").unwrap();
    }

    assert_eq!(load_error(dir.path().to_path_buf()), ErrorName::SpeechEngineUnavailable);
}

#[test]
fn zero_threads_is_rejected() {
    let error = KokoroSpeechEngine::load(&KokoroSettings { num_threads: 0, ..settings(bundled_model_dir()) }).err().unwrap();

    assert_eq!(error.primary(), ErrorName::SpeechEngineUnavailable);
}

#[test]
fn an_empty_provider_is_rejected() {
    let error = KokoroSpeechEngine::load(&KokoroSettings { provider: " ".into(), ..settings(bundled_model_dir()) }).err().unwrap();

    assert_eq!(error.primary(), ErrorName::SpeechEngineUnavailable);
}

#[test]
#[ignore = "loads the real Kokoro model"]
fn unknown_and_non_english_voices_are_rejected() {
    let engine = load(bundled_model_dir());

    for voice in ["nobody", "zf_xiaobei"] {
        let error = engine.synthesize(SpeechRequest { text: "Hello.".into(), voice_id: VoiceId::new(voice) }).unwrap_err();
        assert_eq!(error.primary(), ErrorName::VoiceUnknown, "{voice}");
    }
}

#[test]
#[ignore = "loads the real Kokoro model"]
fn generates_24_khz_speech_and_survives_nul_bytes() {
    let engine = load(bundled_model_dir());
    assert_eq!(engine.voices().len(), 28);

    let audio = engine.synthesize(SpeechRequest { text: "Hello\0 from Lectrice.".into(), voice_id: VoiceId::new("af_heart") }).unwrap();

    assert_eq!(audio.sample_rate, KOKORO_SAMPLE_RATE);
    assert!(audio.samples.len() > KOKORO_SAMPLE_RATE as usize / 2, "expected at least half a second, got {} samples", audio.samples.len());
    assert!(audio.samples.iter().any(|sample| sample.abs() > 0.01));
}

#[test]
#[ignore = "loads the real Kokoro model"]
fn blank_text_gives_empty_audio() {
    let engine = load(bundled_model_dir());

    let audio = engine.synthesize(SpeechRequest { text: " \0 ".into(), voice_id: VoiceId::new("af_heart") }).unwrap();

    assert_eq!((audio.samples.len(), audio.sample_rate), (0, KOKORO_SAMPLE_RATE));
}
