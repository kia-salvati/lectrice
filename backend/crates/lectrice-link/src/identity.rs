//! Each install's X25519 static key pair, the peer ids derived from public keys, and the paired-peer store port.

use std::fmt;

use curve25519_dalek::montgomery::MontgomeryPoint;
use sha2::{Digest, Sha256};
use snow::Builder;

use crate::error::LinkResult;
use crate::noise::{IK_PARAMS, params};

pub const KEY_BYTES: usize = 32;
const PEER_ID_BYTES: usize = 8;

/// This install's static key pair. The shell stores it (the computer: a 0600 file; the phone: Keystore-wrapped).
#[derive(Clone, PartialEq, Eq)]
pub struct StaticKeys {
    private: [u8; KEY_BYTES],
    public: [u8; KEY_BYTES],
}

impl StaticKeys {
    pub fn generate() -> LinkResult<Self> {
        let pair = Builder::new(params(IK_PARAMS)).generate_keypair()?;
        Ok(Self { private: to_key(&pair.private)?, public: to_key(&pair.public)? })
    }

    /// Rebuilds the pair from a stored private key (the public key is derived, so a stored pair can't disagree).
    pub fn from_private(private: [u8; KEY_BYTES]) -> Self {
        let public = MontgomeryPoint::mul_base_clamped(private).to_bytes();
        Self { private, public }
    }

    pub fn private(&self) -> &[u8; KEY_BYTES] {
        &self.private
    }

    pub fn public(&self) -> &[u8; KEY_BYTES] {
        &self.public
    }

    pub fn peer_id(&self) -> PeerId {
        PeerId::of(&self.public)
    }
}

impl fmt::Debug for StaticKeys {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("StaticKeys").field("peer_id", &self.peer_id()).finish_non_exhaustive()
    }
}

fn to_key(bytes: &[u8]) -> LinkResult<[u8; KEY_BYTES]> {
    bytes.try_into().map_err(|_| crate::LinkError::Crypto)
}

/// First 16 hex digits of SHA-256(static public key): how phones recognise a computer in mDNS (`id=`), and how both
/// sides name each other in logs (never by device name).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PeerId(String);

impl PeerId {
    pub fn of(public: &[u8; KEY_BYTES]) -> Self {
        let digest = Sha256::digest(public);
        Self(digest[..PEER_ID_BYTES].iter().map(|byte| format!("{byte:02x}")).collect())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The first 8 hex digits, for log lines.
    pub fn short(&self) -> &str {
        &self.0[..PEER_ID_BYTES]
    }
}

impl fmt::Display for PeerId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// A paired device: public data only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerRecord {
    pub peer_id: PeerId,
    pub name: String,
    pub static_public: [u8; KEY_BYTES],
}

impl PeerRecord {
    pub fn new(name: String, static_public: [u8; KEY_BYTES]) -> Self {
        Self { peer_id: PeerId::of(&static_public), name, static_public }
    }
}

/// The paired devices, kept by the shell (a JSON file on the computer, `peers.json` on the phone).
pub trait PeerStore: Send + Sync {
    fn find(&self, static_public: &[u8; KEY_BYTES]) -> LinkResult<Option<PeerRecord>>;
    /// Adds or replaces (same key) a peer; the store may drop its least recently seen peer to stay within its limit.
    fn put(&self, peer: PeerRecord) -> LinkResult<()>;
    fn remove(&self, peer_id: &PeerId) -> LinkResult<bool>;
}
