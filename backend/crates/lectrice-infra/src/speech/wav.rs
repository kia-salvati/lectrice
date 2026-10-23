use std::io::Cursor;

use hound::{SampleFormat, WavReader, WavSpec, WavWriter};
use lectrice_domain::{AppError, AppResult, Audio, AudioDecoder, AudioEncoder, ErrorName};

const WAV_MIME_TYPE: &str = "audio/wav";
const MONO: u16 = 1;
const BITS_PER_SAMPLE: u16 = 16;
const BYTES_PER_SAMPLE: usize = 2;
const HEADER_BYTES: usize = 44;

/// 16-bit PCM mono WAV, built in memory.
#[derive(Debug, Default, Clone, Copy)]
pub struct WavEncoder;

impl AudioEncoder for WavEncoder {
    fn mime_type(&self) -> &str {
        WAV_MIME_TYPE
    }

    fn encode(&self, audio: &Audio) -> AppResult<Vec<u8>> {
        let spec = WavSpec { channels: MONO, sample_rate: audio.sample_rate, bits_per_sample: BITS_PER_SAMPLE, sample_format: SampleFormat::Int };
        let mut buffer = Cursor::new(Vec::with_capacity(HEADER_BYTES + audio.samples.len() * BYTES_PER_SAMPLE));
        let mut writer = WavWriter::new(&mut buffer, spec).map_err(encoding_failed)?;
        for &sample in &audio.samples {
            writer.write_sample(to_pcm16(sample)).map_err(encoding_failed)?;
        }
        writer.finalize().map_err(encoding_failed)?;
        Ok(buffer.into_inner())
    }
}

/// Reads back what [`WavEncoder`] writes (16-bit PCM mono).
#[derive(Debug, Default, Clone, Copy)]
pub struct WavDecoder;

impl AudioDecoder for WavDecoder {
    fn decode(&self, bytes: &[u8]) -> AppResult<Audio> {
        let reader = WavReader::new(Cursor::new(bytes)).map_err(decoding_failed)?;
        let spec = reader.spec();
        if spec.channels != MONO || spec.bits_per_sample != BITS_PER_SAMPLE || spec.sample_format != SampleFormat::Int {
            return Err(AppError::new(ErrorName::Internal).with_detail(format!("unsupported WAV: {spec:?}")));
        }
        let samples = reader.into_samples::<i16>().map(|sample| sample.map(from_pcm16)).collect::<Result<Vec<f32>, _>>().map_err(decoding_failed)?;
        Ok(Audio { samples, sample_rate: spec.sample_rate })
    }
}

fn to_pcm16(sample: f32) -> i16 {
    (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)).round() as i16
}

fn encoding_failed(error: hound::Error) -> AppError {
    AppError::new(ErrorName::Internal).with_detail("WAV encoding failed").with_source(error)
}

fn from_pcm16(sample: i16) -> f32 {
    f32::from(sample) / f32::from(i16::MAX)
}

fn decoding_failed(error: hound::Error) -> AppError {
    AppError::new(ErrorName::Internal).with_detail("WAV decoding failed").with_source(error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decoding_returns_the_encoded_audio() {
        let audio = Audio { samples: vec![0.0, 0.5, -0.5, 1.0, -1.0], sample_rate: 24_000 };

        let decoded = WavDecoder.decode(&WavEncoder.encode(&audio).unwrap()).unwrap();

        assert_eq!(decoded.sample_rate, 24_000);
        for (got, expected) in decoded.samples.iter().zip(&audio.samples) {
            assert!((got - expected).abs() < 1e-4, "{got} vs {expected}");
        }
        assert_eq!(decoded.samples.len(), audio.samples.len());
    }

    #[test]
    fn garbage_is_an_error() {
        assert_eq!(WavDecoder.decode(b"not a wav").unwrap_err().primary(), ErrorName::Internal);
    }
}
