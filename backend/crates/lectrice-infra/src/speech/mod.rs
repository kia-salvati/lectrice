//! The voice adapter (`KokoroSpeechEngine`, sherpa-onnx; feature `kokoro`) and the WAV encoder and decoder (hound).

#[cfg(feature = "kokoro")]
mod kokoro;
#[cfg(feature = "kokoro")]
mod voices;
mod wav;

#[cfg(feature = "kokoro")]
pub use kokoro::{KokoroSettings, KokoroSpeechEngine};
pub use wav::{WavDecoder, WavEncoder};
#[cfg(feature = "ffmpeg-export")]
pub(crate) use wav::{pcm16_mono, to_pcm16};
