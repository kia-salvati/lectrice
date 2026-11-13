//! A stable-toolchain smoke run of the fuzz targets in `fuzz/` (which need nightly `cargo fuzz`): random and mutated
//! inputs into the frame splitter, both message decoders and the FLAC decoder must never panic or over-allocate.

use lectrice_link::{
    ClientMessage, ClipChunk, Item, Make, ServerMessage, decode_client_message, decode_clip, decode_server_message, encode_clip, split_frames,
};

const RUNS: usize = 20_000;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn bytes(&mut self, max: usize) -> Vec<u8> {
        let length = (self.next() as usize) % (max + 1);
        (0..length).map(|_| self.next() as u8).collect()
    }

    /// Flips, inserts or deletes a few bytes of a valid input.
    fn mutate(&mut self, valid: &[u8]) -> Vec<u8> {
        let mut bytes = valid.to_vec();
        for _ in 0..1 + self.next() % 4 {
            let at = if bytes.is_empty() { 0 } else { (self.next() as usize) % bytes.len() };
            match self.next() % 3 {
                0 if !bytes.is_empty() => bytes[at] ^= 1 << (self.next() % 8),
                1 => bytes.insert(at, self.next() as u8),
                _ if !bytes.is_empty() => {
                    bytes.remove(at);
                }
                _ => {}
            }
        }
        bytes
    }
}

fn valid_client_message() -> Vec<u8> {
    let make = ClientMessage::Make(Make {
        batch_id: 3,
        items: vec![Item { item_id: 1, voice_id: "af_heart".into(), text: "An invented line.".into(), urgent: true }],
    });
    postcard::to_allocvec(&make).unwrap()
}

fn valid_server_message() -> Vec<u8> {
    let clip = ServerMessage::Clip(ClipChunk { item_id: 1, sample_rate: 24_000, samples: 4, make_ms: 2, seq: 0, last: true, flac: vec![1, 2, 3, 4] });
    postcard::to_allocvec(&clip).unwrap()
}

#[test]
fn decoders_survive_random_and_mutated_input() {
    let mut rng = Rng(0xdead_beef_cafe_f00d);
    let client = valid_client_message();
    let server = valid_server_message();
    assert!(decode_client_message(&client).is_ok() && decode_server_message(&server).is_ok());
    for _ in 0..RUNS {
        let random = rng.bytes(512);
        let _ = decode_client_message(&random);
        let _ = decode_server_message(&random);
        let _ = split_frames(&random);
        let _ = decode_client_message(&rng.mutate(&client));
        let _ = decode_server_message(&rng.mutate(&server));
    }
}

#[test]
fn the_flac_decoder_survives_mutated_clips() {
    let mut rng = Rng(0x0123_4567_89ab_cdef);
    let pcm: Vec<i16> = (0..6_000).map(|index| ((index as f32 * 0.03).sin() * 8_000.0) as i16).collect();
    let flac = encode_clip(&pcm, 24_000).unwrap();
    for _ in 0..RUNS / 10 {
        let mutated = rng.mutate(&flac);
        if let Ok(samples) = decode_clip(&mutated, 24_000, 6_000) {
            assert!(samples.len() <= 6_000);
        }
        let _ = decode_clip(&rng.bytes(256), 24_000, 6_000);
    }
}
