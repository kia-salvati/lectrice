#![no_main]

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    let _ = lectrice_link::decode_client_message(data);
    let _ = lectrice_link::decode_server_message(data);
});
