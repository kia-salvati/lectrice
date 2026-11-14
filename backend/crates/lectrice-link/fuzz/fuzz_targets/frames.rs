#![no_main]

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    let _ = lectrice_link::split_frames(data);
});
