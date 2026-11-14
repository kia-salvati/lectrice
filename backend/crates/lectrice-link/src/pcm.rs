//! 16-bit PCM, converted exactly like the core's WAV encoder (`lectrice-infra` `speech/wav.rs`), so a clip made on
//! the computer is stored with the same samples as one the phone made itself.

const FULL_SCALE: f32 = i16::MAX as f32;

/// Clamps to [-1, 1], scales by `i16::MAX` and rounds.
pub fn pcm16_from_f32(sample: f32) -> i16 {
    // The clamp keeps the product within i16, so the cast never saturates.
    #[allow(clippy::cast_possible_truncation)]
    let value = (sample.clamp(-1.0, 1.0) * FULL_SCALE).round() as i16;
    value
}

/// The inverse the WAV decoder uses: `pcm16_from_f32(pcm16_to_f32(x)) == x` for every `x`.
pub fn pcm16_to_f32(sample: i16) -> f32 {
    f32::from(sample) / FULL_SCALE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_every_sample() {
        for sample in i16::MIN..=i16::MAX {
            let back = pcm16_from_f32(pcm16_to_f32(sample));
            // i16::MIN has no positive twin; the encoder clamps it to -i16::MAX.
            let expected = sample.max(-i16::MAX);
            assert_eq!(back, expected);
        }
    }

    #[test]
    fn clamps_out_of_range_samples() {
        assert_eq!(pcm16_from_f32(1.5), i16::MAX);
        assert_eq!(pcm16_from_f32(-1.5), -i16::MAX);
        assert_eq!(pcm16_from_f32(f32::NAN), 0);
    }
}
