use std::path::{Path, PathBuf};

use lectrice_domain::{AppError, AppResult, Audio, ErrorName, SpeechEngine, SpeechRequest, Voice};
use sherpa_onnx::{GenerationConfig, OfflineTts, OfflineTtsConfig, OfflineTtsKokoroModelConfig, OfflineTtsModelConfig};

use super::voices;

const ENGINE_ID: &str = "kokoro-v1.0";
const MODEL_FILE: &str = "model.onnx";
const VOICES_FILE: &str = "voices.bin";
const TOKENS_FILE: &str = "tokens.txt";
/// British voices use the US lexicon too in v1 (spec §10); the lexicon is fixed per engine.
const LEXICON_FILE: &str = "lexicon-us-en.txt";
const ESPEAK_DATA_DIR: &str = "espeak-ng-data";
const ESPEAK_REQUIRED_FILES: [&str; 4] = ["phontab", "phonindex", "phondata", "intonations"];
const NUL: char = '\0';

type NoProgress = fn(&[f32], f32) -> bool;

#[derive(Debug, Clone)]
pub struct KokoroSettings {
    pub model_dir: PathBuf,
    pub num_threads: u32,
    /// The onnxruntime execution provider: "cpu", or e.g. "xnnpack" / "nnapi" on Android. sherpa-onnx falls back to
    /// the CPU (and logs it) when the provider isn't in its onnxruntime build.
    pub provider: String,
}

/// Kokoro 82M through sherpa-onnx. Exposes only the English voices and always speaks at 1x.
pub struct KokoroSpeechEngine {
    tts: OfflineTts,
    sample_rate: u32,
}

impl KokoroSpeechEngine {
    /// Fails with `SpeechEngineUnavailable` instead of creating sherpa-onnx with a bad config, which would exit the process.
    pub fn load(settings: &KokoroSettings) -> AppResult<Self> {
        let config = engine_config(settings)?;
        let Some(tts) = OfflineTts::create(&config) else {
            return Err(unavailable("sherpa-onnx could not create the Kokoro engine"));
        };
        let speakers = usize::try_from(tts.num_speakers()).unwrap_or_default();
        if speakers < voices::count() {
            return Err(unavailable(format!("the model has {speakers} voices, expected at least {}", voices::count())));
        }
        let sample_rate = u32::try_from(tts.sample_rate()).map_err(|_| unavailable("the model reports an invalid sample rate"))?;
        Ok(Self { tts, sample_rate })
    }
}

impl SpeechEngine for KokoroSpeechEngine {
    fn id(&self) -> &str {
        ENGINE_ID
    }

    fn voices(&self) -> Vec<Voice> {
        voices::all()
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn synthesize(&self, request: SpeechRequest) -> AppResult<Audio> {
        let Some(sid) = voices::speaker_id(&request.voice_id) else {
            return Err(AppError::new(ErrorName::VoiceUnknown).with_detail(format!("voice {}", request.voice_id)));
        };
        // sherpa-onnx panics on NUL bytes in the text.
        let text = request.text.replace(NUL, "");
        if text.trim().is_empty() {
            return Ok(Audio { samples: Vec::new(), sample_rate: self.sample_rate });
        }

        let generation = GenerationConfig { sid, ..GenerationConfig::default() };
        let Some(generated) = self.tts.generate_with_config(&text, &generation, None::<NoProgress>) else {
            return Err(AppError::new(ErrorName::SpeechSynthesisFailed).with_detail(format!("sherpa-onnx returned no audio for voice {}", request.voice_id)));
        };
        Ok(Audio { samples: generated.samples().to_vec(), sample_rate: self.sample_rate })
    }
}

fn engine_config(settings: &KokoroSettings) -> AppResult<OfflineTtsConfig> {
    let dir = &settings.model_dir;
    if !dir.is_dir() {
        return Err(unavailable(format!("model directory not found: {}", dir.display())));
    }
    let espeak_dir = dir.join(ESPEAK_DATA_DIR);
    for file in ESPEAK_REQUIRED_FILES {
        existing_file(&espeak_dir.join(file))?;
    }
    let num_threads = i32::try_from(settings.num_threads).ok().filter(|threads| *threads > 0).ok_or_else(|| unavailable("num_threads must be at least 1"))?;
    let provider = settings.provider.trim();
    if provider.is_empty() || provider.contains(NUL) {
        return Err(unavailable("provider must be a non-empty name"));
    }

    let kokoro = OfflineTtsKokoroModelConfig {
        model: Some(existing_file(&dir.join(MODEL_FILE))?),
        voices: Some(existing_file(&dir.join(VOICES_FILE))?),
        tokens: Some(existing_file(&dir.join(TOKENS_FILE))?),
        lexicon: Some(existing_file(&dir.join(LEXICON_FILE))?),
        data_dir: Some(utf8(&espeak_dir)?),
        ..OfflineTtsKokoroModelConfig::default()
    };
    let model = OfflineTtsModelConfig { kokoro, num_threads, provider: Some(provider.to_string()), ..OfflineTtsModelConfig::default() };
    Ok(OfflineTtsConfig { model, ..OfflineTtsConfig::default() })
}

fn existing_file(path: &Path) -> AppResult<String> {
    if !path.is_file() {
        return Err(unavailable(format!("missing model file {}", path.display())));
    }
    utf8(path)
}

fn utf8(path: &Path) -> AppResult<String> {
    path.to_str().map(str::to_owned).ok_or_else(|| unavailable(format!("model path is not UTF-8: {}", path.display())))
}

fn unavailable(detail: impl Into<String>) -> AppError {
    AppError::new(ErrorName::SpeechEngineUnavailable).with_detail(detail)
}
