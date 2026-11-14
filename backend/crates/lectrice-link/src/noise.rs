//! Noise over frames: the handshake steps, then an encrypted message channel. Messages longer than one Noise message
//! are split into fragments (`flag ‖ bytes`, flag 1 = more follow) and joined up to [`MAX_MESSAGE_BYTES`]. Noise's
//! nonces are counters, so a replayed, dropped or re-ordered frame fails to decrypt and ends the session.

use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use snow::{HandshakeState, TransportState};

use crate::error::{LinkError, LinkResult};
use crate::frame::{FrameReader, write_frame};
use crate::limits::{MAX_FRAME_BYTES, MAX_MESSAGE_BYTES};
use crate::sync::lock;

pub(crate) const IK_PARAMS: &str = "Noise_IK_25519_ChaChaPoly_BLAKE2s";
pub(crate) const XXPSK3_PARAMS: &str = "Noise_XXpsk3_25519_ChaChaPoly_BLAKE2s";
pub(crate) const PSK_LOCATION: u8 = 3;
const TAG_BYTES: usize = 16;
const FLAG_BYTES: usize = 1;
const MAX_FRAGMENT_BYTES: usize = MAX_FRAME_BYTES - TAG_BYTES - FLAG_BYTES;
const FLAG_LAST: u8 = 0;
const FLAG_MORE: u8 = 1;

pub(crate) fn params(name: &str) -> snow::params::NoiseParams {
    name.parse().expect("a valid built-in Noise pattern")
}

/// Writes the next handshake message.
pub(crate) fn send_handshake(state: &mut HandshakeState, writer: &mut impl Write, payload: &[u8]) -> LinkResult<()> {
    let mut message = vec![0u8; MAX_FRAME_BYTES];
    let length = state.write_message(payload, &mut message)?;
    write_frame(writer, &message[..length])?;
    Ok(())
}

/// Reads the next handshake message before `deadline`; returns its payload.
pub(crate) fn receive_handshake<R: Read>(state: &mut HandshakeState, reader: &mut FrameReader<R>, deadline: Instant) -> LinkResult<Vec<u8>> {
    let frame = read_frame_by(reader, deadline)?;
    let mut payload = vec![0u8; MAX_FRAME_BYTES];
    let length = state.read_message(&frame, &mut payload)?;
    payload.truncate(length);
    Ok(payload)
}

/// A whole frame before `deadline` (the socket's read timeout is what wakes the loop).
pub(crate) fn read_frame_by<R: Read>(reader: &mut FrameReader<R>, deadline: Instant) -> LinkResult<Vec<u8>> {
    loop {
        match reader.read_frame() {
            Ok(Some(frame)) => return Ok(frame),
            Ok(None) if Instant::now() >= deadline => return Err(LinkError::Timeout("handshake")),
            Ok(None) => {}
            Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => return Err(LinkError::Disconnected),
            Err(error) => return Err(error.into()),
        }
    }
}

/// The encrypted channel after a handshake. One lock for both directions: it is held only while encrypting or
/// decrypting, never while waiting on the socket.
#[derive(Clone)]
pub(crate) struct Channel {
    state: Arc<Mutex<TransportState>>,
}

impl Channel {
    pub(crate) fn new(state: TransportState) -> Self {
        Self { state: Arc::new(Mutex::new(state)) }
    }

    /// The frames that carry `message`.
    pub(crate) fn seal(&self, message: &[u8]) -> LinkResult<Vec<Vec<u8>>> {
        if message.len() > MAX_MESSAGE_BYTES {
            return Err(LinkError::LimitExceeded("message size"));
        }
        let mut state = lock(&self.state);
        let fragments: Vec<&[u8]> = if message.is_empty() { vec![message] } else { message.chunks(MAX_FRAGMENT_BYTES).collect() };
        let count = fragments.len();
        fragments
            .into_iter()
            .enumerate()
            .map(|(index, fragment)| {
                let mut plain = Vec::with_capacity(FLAG_BYTES + fragment.len());
                plain.push(if index + 1 == count { FLAG_LAST } else { FLAG_MORE });
                plain.extend_from_slice(fragment);
                let mut sealed = vec![0u8; plain.len() + TAG_BYTES];
                let length = state.write_message(&plain, &mut sealed)?;
                sealed.truncate(length);
                Ok(sealed)
            })
            .collect()
    }

    pub(crate) fn opener(&self) -> Opener {
        Opener { channel: self.clone(), partial: Vec::new() }
    }
}

/// Joins fragments back into messages.
pub(crate) struct Opener {
    channel: Channel,
    partial: Vec<u8>,
}

impl Opener {
    /// `Some(message)` once its last fragment is in.
    pub(crate) fn open(&mut self, frame: &[u8]) -> LinkResult<Option<Vec<u8>>> {
        let mut plain = vec![0u8; frame.len()];
        let length = {
            let mut state = lock(&self.channel.state);
            state.read_message(frame, &mut plain)?
        };
        let (&flag, fragment) = plain[..length].split_first().ok_or(LinkError::Protocol("empty fragment"))?;
        if self.partial.len() + fragment.len() > MAX_MESSAGE_BYTES {
            return Err(LinkError::LimitExceeded("message size"));
        }
        self.partial.extend_from_slice(fragment);
        match flag {
            FLAG_LAST => Ok(Some(std::mem::take(&mut self.partial))),
            FLAG_MORE => Ok(None),
            _ => Err(LinkError::Protocol("fragment flag")),
        }
    }
}

#[cfg(test)]
mod tests {
    use snow::Builder;

    use super::*;

    fn pair_of_channels() -> (Channel, Channel) {
        let builder = Builder::new(params(IK_PARAMS));
        let initiator_keys = builder.generate_keypair().unwrap();
        let responder_keys = Builder::new(params(IK_PARAMS)).generate_keypair().unwrap();
        let mut initiator = Builder::new(params(IK_PARAMS))
            .local_private_key(&initiator_keys.private)
            .unwrap()
            .remote_public_key(&responder_keys.public)
            .unwrap()
            .build_initiator()
            .unwrap();
        let mut responder = Builder::new(params(IK_PARAMS)).local_private_key(&responder_keys.private).unwrap().build_responder().unwrap();
        let mut buffer = vec![0u8; MAX_FRAME_BYTES];
        let mut payload = vec![0u8; MAX_FRAME_BYTES];
        let length = initiator.write_message(&[], &mut buffer).unwrap();
        responder.read_message(&buffer[..length], &mut payload).unwrap();
        let length = responder.write_message(&[], &mut buffer).unwrap();
        initiator.read_message(&buffer[..length], &mut payload).unwrap();
        (Channel::new(initiator.into_transport_mode().unwrap()), Channel::new(responder.into_transport_mode().unwrap()))
    }

    fn deliver(sender: &Channel, opener: &mut Opener, message: &[u8]) -> Vec<u8> {
        let frames = sender.seal(message).unwrap();
        let mut out = None;
        for frame in frames {
            assert!(frame.len() <= MAX_FRAME_BYTES);
            assert!(out.is_none(), "a message ends with its last fragment");
            out = opener.open(&frame).unwrap();
        }
        out.unwrap()
    }

    #[test]
    fn carries_small_empty_and_multi_fragment_messages() {
        let (phone, computer) = pair_of_channels();
        let mut opener = computer.opener();
        for message in [b"hi".to_vec(), Vec::new(), vec![5u8; 200_000]] {
            assert_eq!(deliver(&phone, &mut opener, &message), message);
        }
    }

    #[test]
    fn a_replayed_or_reordered_frame_fails() {
        let (phone, computer) = pair_of_channels();
        let mut opener = computer.opener();
        let first = phone.seal(b"one").unwrap().remove(0);
        let second = phone.seal(b"two").unwrap().remove(0);
        assert!(opener.open(&second).is_err(), "out of order");
        assert_eq!(opener.open(&first).unwrap().unwrap(), b"one");
        assert!(opener.open(&first).is_err(), "replayed");
        let mut other_session = pair_of_channels().1.opener();
        assert!(other_session.open(&first).is_err(), "another session's frame");
    }

    #[test]
    fn a_tampered_frame_fails() {
        let (phone, computer) = pair_of_channels();
        let mut frame = phone.seal(b"payload").unwrap().remove(0);
        frame[3] ^= 1;
        assert!(matches!(computer.opener().open(&frame), Err(LinkError::Crypto)));
    }
}
