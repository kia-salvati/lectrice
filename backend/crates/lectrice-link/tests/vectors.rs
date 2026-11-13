//! Crypto vectors (file 24 §13): the Noise patterns this link uses, run through the same `snow` build, must give the
//! standard cacophony results; SPAKE2 with a fixed RNG must keep giving the recorded key (a change in the crate's
//! maths or hashing would silently break pairing between app versions).

use rand_core::{CryptoRng, RngCore};
use serde_json::Value;
use snow::Builder;
use spake2::{Ed25519Group, Identity, Password, Spake2};

const NOISE_VECTORS: &str = include_str!("vectors/noise.json");

fn hex(text: &str) -> Vec<u8> {
    (0..text.len()).step_by(2).map(|at| u8::from_str_radix(&text[at..at + 2], 16).unwrap()).collect()
}

fn field(vector: &Value, name: &str) -> Option<Vec<u8>> {
    vector.get(name).and_then(Value::as_str).map(hex)
}

fn run_vector(vector: &Value) {
    let name = vector["protocol_name"].as_str().unwrap();
    let params = name.parse().unwrap();
    let (init_psk, resp_psk) =
        (vector.get("init_psks").map(|psks| hex(psks[0].as_str().unwrap())), vector.get("resp_psks").map(|psks| hex(psks[0].as_str().unwrap())));
    let init_static = field(vector, "init_static").unwrap();
    let init_ephemeral = field(vector, "init_ephemeral").unwrap();
    let init_prologue = field(vector, "init_prologue").unwrap();
    let mut init =
        Builder::new(params).local_private_key(&init_static).unwrap().fixed_ephemeral_key_for_testing_only(&init_ephemeral).prologue(&init_prologue).unwrap();
    let remote = field(vector, "init_remote_static");
    if let Some(remote) = &remote {
        init = init.remote_public_key(remote).unwrap();
    }
    let init_psk_key: Option<[u8; 32]> = init_psk.map(|psk| psk.try_into().unwrap());
    if let Some(psk) = &init_psk_key {
        init = init.psk(3, psk).unwrap();
    }
    let mut initiator = init.build_initiator().unwrap();

    let resp_static = field(vector, "resp_static").unwrap();
    let resp_ephemeral = field(vector, "resp_ephemeral").unwrap();
    let resp_prologue = field(vector, "resp_prologue").unwrap();
    let params = name.parse().unwrap();
    let mut resp =
        Builder::new(params).local_private_key(&resp_static).unwrap().fixed_ephemeral_key_for_testing_only(&resp_ephemeral).prologue(&resp_prologue).unwrap();
    let resp_psk_key: Option<[u8; 32]> = resp_psk.map(|psk| psk.try_into().unwrap());
    if let Some(psk) = &resp_psk_key {
        resp = resp.psk(3, psk).unwrap();
    }
    let mut responder = resp.build_responder().unwrap();

    let messages = vector["messages"].as_array().unwrap();
    let mut buffer = vec![0u8; 65_535];
    let mut payload = vec![0u8; 65_535];
    let mut transports = None;
    for (index, message) in messages.iter().enumerate() {
        let expected_payload = hex(message["payload"].as_str().unwrap());
        let expected_cipher = hex(message["ciphertext"].as_str().unwrap());
        let initiator_sends = index % 2 == 0;
        if transports.is_none() && !initiator.is_handshake_finished() {
            let (sender, receiver) = if initiator_sends { (&mut initiator, &mut responder) } else { (&mut responder, &mut initiator) };
            let length = sender.write_message(&expected_payload, &mut buffer).unwrap();
            assert_eq!(&buffer[..length], &expected_cipher[..], "{name} handshake message {index}");
            let read = receiver.read_message(&expected_cipher, &mut payload).unwrap();
            assert_eq!(&payload[..read], &expected_payload[..]);
            if initiator.is_handshake_finished() {
                assert_eq!(initiator.get_handshake_hash(), &hex(vector["handshake_hash"].as_str().unwrap())[..], "{name} handshake hash");
            }
            continue;
        }
        let (mut init_t, mut resp_t) = match transports.take() {
            Some(pair) => pair,
            None => {
                let init_t = std::mem::replace(&mut initiator, Builder::new("Noise_NN_25519_ChaChaPoly_BLAKE2s".parse().unwrap()).build_initiator().unwrap())
                    .into_transport_mode()
                    .unwrap();
                let resp_t = std::mem::replace(&mut responder, Builder::new("Noise_NN_25519_ChaChaPoly_BLAKE2s".parse().unwrap()).build_responder().unwrap())
                    .into_transport_mode()
                    .unwrap();
                (init_t, resp_t)
            }
        };
        let (sender, receiver) = if initiator_sends { (&mut init_t, &mut resp_t) } else { (&mut resp_t, &mut init_t) };
        let length = sender.write_message(&expected_payload, &mut buffer).unwrap();
        assert_eq!(&buffer[..length], &expected_cipher[..], "{name} transport message {index}");
        let read = receiver.read_message(&expected_cipher, &mut payload).unwrap();
        assert_eq!(&payload[..read], &expected_payload[..]);
        transports = Some((init_t, resp_t));
    }
    assert!(transports.is_some(), "{name}: the vector reached transport messages");
}

#[test]
fn noise_ik_and_xxpsk3_match_the_cacophony_vectors() {
    let file: Value = serde_json::from_str(NOISE_VECTORS).unwrap();
    let vectors = file["vectors"].as_array().unwrap();
    let names: Vec<&str> = vectors.iter().map(|vector| vector["protocol_name"].as_str().unwrap()).collect();
    assert_eq!(names, ["Noise_IK_25519_ChaChaPoly_BLAKE2s", "Noise_XXpsk3_25519_ChaChaPoly_BLAKE2s"]);
    vectors.iter().for_each(run_vector);
}

/// Bytes from a fixed counter: deterministic, for the recorded vector only.
struct FixedRng(u8);

impl RngCore for FixedRng {
    fn next_u32(&mut self) -> u32 {
        rand_core::impls::next_u32_via_fill(self)
    }
    fn next_u64(&mut self) -> u64 {
        rand_core::impls::next_u64_via_fill(self)
    }
    fn fill_bytes(&mut self, dest: &mut [u8]) {
        for byte in dest {
            *byte = self.0;
            self.0 = self.0.wrapping_add(1);
        }
    }
    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand_core::Error> {
        self.fill_bytes(dest);
        Ok(())
    }
}

impl CryptoRng for FixedRng {}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[test]
fn spake2_with_our_identities_keeps_the_recorded_key() {
    let password = Password::new(b"482913");
    let (phone, phone_message) =
        Spake2::<Ed25519Group>::start_a_with_rng(&password, &Identity::new(b"lectrice-phone"), &Identity::new(b"lectrice-desktop"), FixedRng(1));
    let (computer, computer_message) =
        Spake2::<Ed25519Group>::start_b_with_rng(&password, &Identity::new(b"lectrice-phone"), &Identity::new(b"lectrice-desktop"), FixedRng(101));
    let phone_key = phone.finish(&computer_message).unwrap();
    let computer_key = computer.finish(&phone_message).unwrap();
    assert_eq!(phone_key, computer_key);
    assert_eq!(to_hex(&phone_message), RECORDED_PHONE_MESSAGE);
    assert_eq!(to_hex(&phone_key), RECORDED_KEY);
}

#[test]
fn spake2_with_a_different_code_gives_a_different_key() {
    let ids = (Identity::new(b"lectrice-phone"), Identity::new(b"lectrice-desktop"));
    let (phone, phone_message) = Spake2::<Ed25519Group>::start_a_with_rng(&Password::new(b"482913"), &ids.0, &ids.1, FixedRng(1));
    let (computer, computer_message) = Spake2::<Ed25519Group>::start_b_with_rng(&Password::new(b"482914"), &ids.0, &ids.1, FixedRng(101));
    assert_ne!(phone.finish(&computer_message).unwrap(), computer.finish(&phone_message).unwrap());
}

/// The upstream known-answer vector (python-spake2 interop, from the spake2 crate's own test) can't be replayed
/// through the public API; these are recorded from spake2 0.4.0 on 2026-11-13.
const RECORDED_PHONE_MESSAGE: &str = "41f71195ffe4de312930a8e4dfcb1125c395b5fe14e4f39567023ae92d5e223c72";
const RECORDED_KEY: &str = "e32e88b670732443af7a64f01cd07025598e5e8c841cee2a75c239d1f4b675f7";
