//! The helper link between a phone and a paired computer on the same home network (file 24, C40 and C41).
//!
//! The phone (the `client` feature) sends sentence items; the computer (the `server` feature) turns them into speech
//! and sends the clips back as FLAC. Pairing is a 6-digit code checked with SPAKE2 and Noise `XXpsk3`; every later
//! connection is Noise `IK` with both static keys pinned. Nothing here knows about the speech engine, the audio cache
//! or the platform: the shells plug in through [`server::EngineProvider`], [`PeerStore`] and their own key storage.

// The transport code is shared by the phone's and the computer's side; a build with neither feature (nothing ships one)
// leaves it unused.
#![cfg_attr(not(any(feature = "client", feature = "server")), allow(dead_code, unused_imports))]

mod codec;
mod error;
mod flac;
mod frame;
mod identity;
pub mod limits;
mod messages;
mod net;
mod noise;
mod pairing;
mod pcm;
mod socket;
mod sync;

#[cfg(feature = "client")]
pub mod client;
#[cfg(feature = "server")]
pub mod server;
#[cfg(feature = "testing")]
pub mod testing;

pub use error::{LinkError, LinkErrorName, LinkResult, PairingFailure};
pub use flac::{MIN_CLIP_SAMPLES, decode_clip, encode_clip};
pub use frame::split_frames;
pub use identity::{PeerId, PeerRecord, PeerStore, StaticKeys};
pub use messages::{
    ByeReason, CancelScope, ClientMessage, ClipChunk, EngineOffer, Hello, Item, ItemError, Load, Make, Open, PairHello, Ready, Refusal, ServerMessage, Welcome,
    speakable_text,
};
pub use net::{is_allowed_peer, is_lan_address};
pub use pairing::{Paired, PairingCode};
pub use pcm::{pcm16_from_f32, pcm16_to_f32};

/// Decodes and bounds-checks a message the phone sent (the server's view; also a fuzz target).
pub fn decode_client_message(bytes: &[u8]) -> LinkResult<ClientMessage> {
    codec::decode(bytes)
}

/// Decodes and bounds-checks a message the computer sent (the phone's view; also a fuzz target).
pub fn decode_server_message(bytes: &[u8]) -> LinkResult<ServerMessage> {
    codec::decode(bytes)
}
