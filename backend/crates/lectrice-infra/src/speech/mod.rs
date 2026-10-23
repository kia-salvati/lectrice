//! The voice adapter (`KokoroSpeechEngine`, sherpa-onnx) and the WAV encoder and decoder (hound).

mod kokoro;
mod voices;
mod wav;

pub use kokoro::{KokoroSettings, KokoroSpeechEngine};
pub use wav::{WavDecoder, WavEncoder};
