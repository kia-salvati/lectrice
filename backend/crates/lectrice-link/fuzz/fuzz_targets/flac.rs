#![no_main]

const MAX_SAMPLES: u64 = 24_000 * 120;

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    if let Ok(samples) = lectrice_link::decode_clip(data, 24_000, MAX_SAMPLES) {
        assert!(samples.len() as u64 <= MAX_SAMPLES);
    }
});
