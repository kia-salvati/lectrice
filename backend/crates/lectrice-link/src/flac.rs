//! Clips on the wire are FLAC (lossless) of the 16-bit mono PCM the audio cache stores. Encoder:
//! `flacenc` (pure Rust); decoder: `claxon` (pure Rust), with hard bounds on what it accepts.

use flacenc::bitsink::ByteSink;
use flacenc::component::BitRepr;
use flacenc::error::Verify;

use crate::error::{LinkError, LinkResult};
use crate::limits::{MAX_CLIP_SECONDS, MAX_SAMPLE_RATE};

const CHANNELS: usize = 1;
const BITS_PER_SAMPLE: usize = 16;
/// FLAC's smallest block; decoders reject a stream whose only block is shorter. The server pads shorter clips.
pub const MIN_CLIP_SAMPLES: usize = 16;
const LONGEST_CLIP_SAMPLES: u64 = MAX_CLIP_SECONDS as u64 * MAX_SAMPLE_RATE as u64;

/// Encodes mono 16-bit samples at `sample_rate` (flacenc's default settings, single-threaded).
pub fn encode_clip(samples: &[i16], sample_rate: u32) -> LinkResult<Vec<u8>> {
    if samples.len() < MIN_CLIP_SAMPLES {
        return Err(LinkError::Codec("clip shorter than one FLAC block".into()));
    }
    let config = flacenc::config::Encoder::default().into_verified().map_err(|(_, error)| LinkError::Codec(error.to_string()))?;
    let wide: Vec<i32> = samples.iter().map(|&sample| i32::from(sample)).collect();
    let rate = usize::try_from(sample_rate).map_err(|_| LinkError::Codec("sample rate".into()))?;
    let source = flacenc::source::MemSource::from_samples(&wide, CHANNELS, BITS_PER_SAMPLE, rate);
    let mut stream = flacenc::encode_with_fixed_block_size(&config, source, config.block_size).map_err(|error| LinkError::Codec(error.to_string()))?;
    // flacenc 0.5.1 writes the short last block's size as STREAMINFO's minimum, and decoders refuse a minimum under 16
    // (clips whose length mod 4096 is 1–15). A fixed-block stream may state the full size for both.
    let max_block = stream.stream_info().max_block_size();
    stream.stream_info_mut().set_block_sizes(max_block, max_block).map_err(|error| LinkError::Codec(error.to_string()))?;
    let mut sink = ByteSink::new();
    stream.write(&mut sink).map_err(|error| LinkError::Codec(error.to_string()))?;
    Ok(sink.into_inner())
}

/// Decodes a clip, refusing anything but mono 16-bit at `sample_rate` with at most `max_samples` samples (and never
/// more than the longest clip the protocol allows, whatever the caller asks for).
pub fn decode_clip(flac: &[u8], sample_rate: u32, max_samples: u64) -> LinkResult<Vec<i16>> {
    let max_samples = max_samples.min(LONGEST_CLIP_SAMPLES);
    let mut reader = claxon::FlacReader::new(flac).map_err(|error| LinkError::Codec(error.to_string()))?;
    let info = reader.streaminfo();
    if info.channels as usize != CHANNELS || info.bits_per_sample as usize != BITS_PER_SAMPLE || info.sample_rate != sample_rate {
        return Err(LinkError::Protocol("clip format"));
    }
    if info.samples.is_some_and(|samples| samples > max_samples) {
        return Err(LinkError::LimitExceeded("clip length"));
    }
    let mut out = Vec::with_capacity(usize::try_from(info.samples.unwrap_or(0).min(max_samples)).unwrap_or(0));
    for sample in reader.samples() {
        let sample = sample.map_err(|error| LinkError::Codec(error.to_string()))?;
        if out.len() as u64 >= max_samples {
            return Err(LinkError::LimitExceeded("clip length"));
        }
        out.push(i16::try_from(sample).map_err(|_| LinkError::Protocol("sample range"))?);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 24_000;

    /// A speech-like test signal: a few sliding tones with an envelope, plus quiet noise.
    fn signal(samples: usize) -> Vec<i16> {
        let mut seed = 0x2545_f491_u32;
        (0..samples)
            .map(|index| {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                let time = index as f32 / RATE as f32;
                let envelope = (time * 3.0).sin().abs();
                let tone = (time * (180.0 + 40.0 * (time * 0.7).sin()) * std::f32::consts::TAU).sin();
                let noise = (seed % 200) as f32 - 100.0;
                (tone * envelope * 9_000.0 + noise) as i16
            })
            .collect()
    }

    #[test]
    fn round_trips_bit_exact_and_compresses() {
        let pcm = signal(RATE as usize * 3 + 123);
        let flac = encode_clip(&pcm, RATE).unwrap();
        assert!(flac.len() < pcm.len() * 2, "{} bytes for {} samples", flac.len(), pcm.len());
        assert_eq!(decode_clip(&flac, RATE, pcm.len() as u64).unwrap(), pcm);
    }

    #[test]
    fn handles_short_and_extreme_clips() {
        // 4101, 8193 and 4111 samples: a short last block (the flacenc STREAMINFO case).
        for pcm in [vec![0i16; MIN_CLIP_SAMPLES], vec![i16::MAX; 5000], signal(4101), signal(8193), signal(4111), vec![i16::MIN; 4096 + MIN_CLIP_SAMPLES]] {
            let flac = encode_clip(&pcm, RATE).unwrap();
            assert_eq!(decode_clip(&flac, RATE, 10_000).unwrap(), pcm);
        }
        assert!(encode_clip(&[0; MIN_CLIP_SAMPLES - 1], RATE).is_err());
    }

    #[test]
    fn refuses_wrong_rate_too_long_and_garbage() {
        let flac = encode_clip(&signal(4800), RATE).unwrap();
        assert!(matches!(decode_clip(&flac, 22_050, 10_000), Err(LinkError::Protocol(_))));
        assert!(matches!(decode_clip(&flac, RATE, 4799), Err(LinkError::LimitExceeded(_))));
        assert!(decode_clip(b"not flac at all", RATE, 10_000).is_err());
        assert!(decode_clip(&flac[..flac.len() / 2], RATE, 10_000).is_err());
    }
}
