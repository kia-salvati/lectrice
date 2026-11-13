//! The voice adapter (`KokoroSpeechEngine`, sherpa-onnx; feature `kokoro`) and the WAV encoder and decoder (hound).

mod deferred;
#[cfg(feature = "kokoro")]
mod kokoro;
#[cfg(feature = "kokoro")]
mod voices;
mod wav;

pub use deferred::{DeferredEngineInfo, DeferredSpeechEngine};
#[cfg(feature = "kokoro")]
pub use kokoro::{KokoroSettings, KokoroSpeechEngine, KokoroVariant};
pub use wav::{WavDecoder, WavEncoder};
#[cfg(feature = "ffmpeg-export")]
pub(crate) use wav::{pcm16_mono, to_pcm16};
