//! Pairing (file 24 §3.2, C41): SPAKE2 on the 6-digit code gives a key `K`; Noise `XXpsk3` with
//! `psk = HKDF(K, "lectrice-pair-v1")` then carries both static keys, authenticated by the code. The code itself never
//! crosses the network, and a recorded attempt can't be brute-forced offline: each attempt is one online guess.

use std::fmt;
use std::io::{Read, Write};
use std::time::{Duration, Instant};

use hkdf::Hkdf;
use sha2::Sha256;
use snow::Builder;
use spake2::{Ed25519Group, Identity, Password, Spake2};

use crate::error::{LinkError, LinkResult};
use crate::frame::{FrameReader, write_frame};
use crate::identity::{KEY_BYTES, PeerRecord, StaticKeys};
use crate::limits::{PAIR_CODE_DIGITS, PAIR_PROLOGUE, PAIR_PSK_INFO, SPAKE2_ID_COMPUTER, SPAKE2_ID_PHONE};
use crate::messages::{PairHello, Validate};
use crate::noise::{Channel, PSK_LOCATION, XXPSK3_PARAMS, params, read_frame_by, receive_handshake, send_handshake};
use crate::socket::{receive_by, send};

const CODE_SPACE: u32 = 1_000_000;
/// The largest multiple of `CODE_SPACE` below 2³², so taking the remainder gives every code the same chance.
const UNBIASED_LIMIT: u32 = u32::MAX - u32::MAX % CODE_SPACE;
const PSK_BYTES: usize = 32;

/// Six decimal digits, shown on the computer and typed on the phone.
#[derive(Clone, PartialEq, Eq)]
pub struct PairingCode(String);

impl PairingCode {
    pub fn generate() -> LinkResult<Self> {
        loop {
            let mut bytes = [0u8; 4];
            getrandom::fill(&mut bytes).map_err(|_| LinkError::Crypto)?;
            let value = u32::from_le_bytes(bytes);
            if value < UNBIASED_LIMIT {
                return Ok(Self(format!("{:0width$}", value % CODE_SPACE, width = PAIR_CODE_DIGITS)));
            }
        }
    }

    /// Accepts what the user typed: six digits, spaces ignored.
    pub fn parse(typed: &str) -> Option<Self> {
        let digits: String = typed.chars().filter(|character| !character.is_whitespace()).collect();
        (digits.len() == PAIR_CODE_DIGITS && digits.bytes().all(|byte| byte.is_ascii_digit())).then_some(Self(digits))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Never printed: the code is a secret while it is valid.
impl fmt::Debug for PairingCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PairingCode(******)")
    }
}

/// What a finished pairing gives each side: the other device's pinned key and its hello.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paired {
    pub peer: PeerRecord,
    pub hello: PairHello,
}

fn spake_password(code: &PairingCode) -> Password {
    Password::new(code.as_str().as_bytes())
}

fn psk_from(key: &[u8]) -> [u8; PSK_BYTES] {
    let mut psk = [0u8; PSK_BYTES];
    Hkdf::<Sha256>::new(None, key).expand(PAIR_PSK_INFO, &mut psk).expect("32 bytes is a valid HKDF-SHA256 length");
    psk
}

/// The Noise prologue binds the plaintext prologue line and both SPAKE2 messages into the handshake.
fn transcript(message_a: &[u8], message_b: &[u8]) -> Vec<u8> {
    [PAIR_PROLOGUE, message_a, message_b].concat()
}

fn remote_key(state: &snow::HandshakeState) -> LinkResult<[u8; KEY_BYTES]> {
    state.get_remote_static().ok_or(LinkError::Crypto)?.try_into().map_err(|_| LinkError::Crypto)
}

fn checked(hello: PairHello) -> LinkResult<PairHello> {
    hello.validate()?;
    Ok(hello)
}

/// The phone's side, after the TCP connect: sends the prologue, runs SPAKE2 and the handshake, swaps hellos.
pub(crate) fn pair_as_phone<R: Read, W: Write>(
    reader: &mut FrameReader<R>,
    writer: &mut W,
    code: &PairingCode,
    keys: &StaticKeys,
    hello: &PairHello,
    deadline: Instant,
) -> LinkResult<Paired> {
    writer.write_all(PAIR_PROLOGUE)?;
    let (spake, message_a) = Spake2::<Ed25519Group>::start_a(&spake_password(code), &Identity::new(SPAKE2_ID_PHONE), &Identity::new(SPAKE2_ID_COMPUTER));
    write_frame(writer, &message_a)?;
    // The computer closes at once when no pairing code is showing.
    let message_b = read_frame_by(reader, deadline).map_err(|error| match error {
        LinkError::Disconnected => LinkError::PairingFailed("no pairing code is showing on the computer"),
        other => other,
    })?;
    let key = spake.finish(&message_b).map_err(|_| LinkError::Protocol("SPAKE2 message"))?;
    let psk = psk_from(&key);

    let mut state = Builder::new(params(XXPSK3_PARAMS))
        .local_private_key(keys.private())?
        .prologue(&transcript(&message_a, &message_b))?
        .psk(PSK_LOCATION, &psk)?
        .build_initiator()?;
    send_handshake(&mut state, writer, &[])?;
    receive_handshake(&mut state, reader, deadline)?;
    send_handshake(&mut state, writer, &[])?;
    let computer_key = remote_key(&state)?;
    let channel = Channel::new(state.into_transport_mode()?);

    send(&channel, writer, hello)?;
    // A wrong code shows up here: the computer can't read our last handshake message and closes.
    let computer_hello: PairHello = receive_by(&mut channel.opener(), reader, deadline).map_err(|error| match error {
        LinkError::Disconnected | LinkError::Crypto => LinkError::PairingFailed("wrong code"),
        // The computer closes with our hello unread, which some systems report as a reset.
        LinkError::Io(error) if matches!(error.kind(), std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::ConnectionAborted) => {
            LinkError::PairingFailed("wrong code")
        }
        other => other,
    })?;
    let computer_hello = checked(computer_hello)?;
    Ok(Paired { peer: PeerRecord::new(computer_hello.device_name.clone(), computer_key), hello: computer_hello })
}

/// The computer's side, after it read the pair prologue while its code was showing.
/// Each step must arrive within `step` (a stalled attempt frees its slot quickly); only
/// `PairingFailed("wrong code")` means a guess was made.
pub(crate) fn pair_as_computer<R: Read, W: Write>(
    reader: &mut FrameReader<R>,
    writer: &mut W,
    code: &PairingCode,
    keys: &StaticKeys,
    hello: &PairHello,
    step: Duration,
) -> LinkResult<Paired> {
    let by = || Instant::now() + step;
    let message_a = read_frame_by(reader, by())?;
    let (spake, message_b) = Spake2::<Ed25519Group>::start_b(&spake_password(code), &Identity::new(SPAKE2_ID_PHONE), &Identity::new(SPAKE2_ID_COMPUTER));
    write_frame(writer, &message_b)?;
    let key = spake.finish(&message_a).map_err(|_| LinkError::Protocol("SPAKE2 message"))?;
    let psk = psk_from(&key);

    let mut state = Builder::new(params(XXPSK3_PARAMS))
        .local_private_key(keys.private())?
        .prologue(&transcript(&message_a, &message_b))?
        .psk(PSK_LOCATION, &psk)?
        .build_responder()?;
    receive_handshake(&mut state, reader, by())?;
    send_handshake(&mut state, writer, &[])?;
    receive_handshake(&mut state, reader, by()).map_err(|error| match error {
        LinkError::Crypto => LinkError::PairingFailed("wrong code"),
        other => other,
    })?;
    let phone_key = remote_key(&state)?;
    let channel = Channel::new(state.into_transport_mode()?);

    let phone_hello: PairHello = receive_by(&mut channel.opener(), reader, by())?;
    let phone_hello = checked(phone_hello)?;
    send(&channel, writer, hello)?;
    Ok(Paired { peer: PeerRecord::new(phone_hello.device_name.clone(), phone_key), hello: phone_hello })
}

#[cfg(test)]
mod tests {
    use std::os::unix::net::UnixStream;
    use std::thread;

    use super::*;

    fn hello(name: &str) -> PairHello {
        PairHello { device_name: name.into(), app_version: "0.4.0".into(), proto_minor: 0 }
    }

    fn run(phone_code: &str, computer_code: &str) -> (LinkResult<Paired>, LinkResult<Paired>, StaticKeys, StaticKeys) {
        let (phone_end, computer_end) = UnixStream::pair().unwrap();
        for end in [&phone_end, &computer_end] {
            end.set_read_timeout(Some(Duration::from_millis(50))).unwrap();
        }
        let phone_keys = StaticKeys::generate().unwrap();
        let computer_keys = StaticKeys::generate().unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let computer_code = PairingCode::parse(computer_code).unwrap();
        let keys = computer_keys.clone();
        let computer = thread::spawn(move || {
            let mut writer = computer_end.try_clone().unwrap();
            let mut reader = FrameReader::new(computer_end);
            let mut prologue = [0u8; PAIR_PROLOGUE.len()];
            reader_read_exact(&mut reader, &mut prologue);
            assert_eq!(prologue, PAIR_PROLOGUE);
            let result = pair_as_computer(&mut reader, &mut writer, &computer_code, &keys, &hello("Laptop"), Duration::from_secs(2));
            drop(writer);
            result
        });
        let mut writer = phone_end.try_clone().unwrap();
        let mut reader = FrameReader::new(phone_end);
        let phone = pair_as_phone(&mut reader, &mut writer, &PairingCode::parse(phone_code).unwrap(), &phone_keys, &hello("Phone"), deadline);
        drop((reader, writer));
        (phone, computer.join().unwrap(), phone_keys, computer_keys)
    }

    /// The prologue is raw bytes before the first frame.
    fn reader_read_exact(reader: &mut FrameReader<UnixStream>, buffer: &mut [u8]) {
        let mut stream = reader.get_ref();
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut filled = 0;
        while filled < buffer.len() {
            match stream.read(&mut buffer[filled..]) {
                Ok(read) => filled += read,
                Err(_) if Instant::now() < deadline => {}
                Err(error) => panic!("{error}"),
            }
        }
    }

    #[test]
    fn the_right_code_pins_both_keys() {
        let (phone, computer, phone_keys, computer_keys) = run("482913", "482913");
        let phone = phone.unwrap();
        let computer = computer.unwrap();
        assert_eq!(phone.peer.static_public, *computer_keys.public());
        assert_eq!(phone.hello.device_name, "Laptop");
        assert_eq!(computer.peer.static_public, *phone_keys.public());
        assert_eq!(computer.peer.peer_id, phone_keys.peer_id());
    }

    #[test]
    fn a_wrong_code_fails_on_both_sides_at_the_psk_step() {
        let (phone, computer, _, _) = run("482914", "482913");
        assert!(matches!(phone, Err(LinkError::PairingFailed("wrong code"))), "{phone:?}");
        assert!(matches!(computer, Err(LinkError::PairingFailed("wrong code"))), "{computer:?}");
    }

    #[test]
    fn codes_are_six_digits_and_parse_typed_input() {
        for _ in 0..200 {
            let code = PairingCode::generate().unwrap();
            assert_eq!(code.as_str().len(), 6);
            assert!(code.as_str().bytes().all(|byte| byte.is_ascii_digit()));
        }
        assert_eq!(PairingCode::parse(" 482 913 ").unwrap().as_str(), "482913");
        assert!(PairingCode::parse("48291").is_none());
        assert!(PairingCode::parse("48291a").is_none());
        assert!(PairingCode::parse("4829134").is_none());
        assert_eq!(format!("{:?}", PairingCode::parse("482913").unwrap()), "PairingCode(******)");
    }
}
