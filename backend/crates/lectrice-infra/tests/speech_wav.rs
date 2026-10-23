use std::io::Cursor;

use hound::{SampleFormat, WavReader};
use lectrice_domain::{Audio, AudioEncoder};
use lectrice_infra::WavEncoder;

const KOKORO_SAMPLE_RATE: u32 = 24_000;

#[test]
fn writes_a_16_bit_mono_wav_with_the_audio_sample_rate() {
    let audio = Audio { samples: vec![0.0, 0.5, -0.5, 1.0, -1.0], sample_rate: KOKORO_SAMPLE_RATE };

    let bytes = WavEncoder.encode(&audio).unwrap();

    assert_eq!(&bytes[0..4], b"RIFF");
    assert_eq!(&bytes[8..12], b"WAVE");
    let reader = WavReader::new(Cursor::new(bytes)).unwrap();
    let spec = reader.spec();
    assert_eq!((spec.channels, spec.bits_per_sample, spec.sample_format, spec.sample_rate), (1, 16, SampleFormat::Int, KOKORO_SAMPLE_RATE));
    let samples: Vec<i16> = reader.into_samples().map(Result::unwrap).collect();
    assert_eq!(samples, vec![0, 16_384, -16_384, i16::MAX, -i16::MAX]);
}

#[test]
fn clips_out_of_range_samples() {
    let audio = Audio { samples: vec![2.0, -3.0], sample_rate: KOKORO_SAMPLE_RATE };

    let reader = WavReader::new(Cursor::new(WavEncoder.encode(&audio).unwrap())).unwrap();

    let samples: Vec<i16> = reader.into_samples().map(Result::unwrap).collect();
    assert_eq!(samples, vec![i16::MAX, -i16::MAX]);
}

#[test]
fn reports_the_wav_mime_type() {
    assert_eq!(WavEncoder.mime_type(), "audio/wav");
}
