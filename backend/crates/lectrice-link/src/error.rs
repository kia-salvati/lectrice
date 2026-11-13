//! The link's errors. Each maps to one of the registry names 5001–5006 (file 24 C47) that the shells show and log.

use std::io;

use thiserror::Error;

use crate::messages::ByeReason;

pub type LinkResult<T> = Result<T, LinkError>;

/// The registry names the link's errors end in, with the core registry's codes (`lectrice-domain` `ErrorName`; this
/// crate depends on no other Lectrice crate, so the names are mirrored here and a test in `lectrice-infra` checks them).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LinkErrorName {
    HelperNotPaired,
    HelperPairingFailed,
    HelperUnreachable,
    HelperModelMissing,
    HelperIncompatible,
    /// A phone over its item budget (security review S15); sent as `ItemFailed`, never a link error.
    HelperBusy,
}

impl LinkErrorName {
    pub const ALL: [Self; 6] =
        [Self::HelperNotPaired, Self::HelperPairingFailed, Self::HelperUnreachable, Self::HelperModelMissing, Self::HelperIncompatible, Self::HelperBusy];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::HelperNotPaired => "HelperNotPaired",
            Self::HelperPairingFailed => "HelperPairingFailed",
            Self::HelperUnreachable => "HelperUnreachable",
            Self::HelperModelMissing => "HelperModelMissing",
            Self::HelperIncompatible => "HelperIncompatible",
            Self::HelperBusy => "HelperBusy",
        }
    }

    pub const fn code(self) -> u32 {
        match self {
            Self::HelperNotPaired => 5001,
            Self::HelperPairingFailed => 5002,
            Self::HelperUnreachable => 5003,
            Self::HelperModelMissing => 5004,
            Self::HelperIncompatible => 5005,
            Self::HelperBusy => 5006,
        }
    }
}

#[derive(Debug, Error)]
pub enum LinkError {
    /// The other side doesn't know this device's key (forgotten there).
    #[error("this device isn't paired with the other side")]
    NotPaired,
    /// The code was wrong, or the pairing window isn't open, or it closed.
    #[error("pairing failed: {0}")]
    PairingFailed(&'static str),
    #[error("the computer doesn't have the voice model {bundle_id}")]
    ModelMissing { bundle_id: String },
    /// Another protocol major, or the same model with another front-end revision.
    #[error("incompatible: {0}")]
    Incompatible(&'static str),
    /// The address isn't on the home network (private or link-local), so the link refuses it.
    #[error("the address isn't on the local network")]
    NotLocal,
    #[error("the other side sent something invalid: {0}")]
    Protocol(&'static str),
    #[error("a limit was exceeded: {0}")]
    LimitExceeded(&'static str),
    /// Authentication or decryption failed.
    #[error("the encrypted channel failed")]
    Crypto,
    #[error("timed out: {0}")]
    Timeout(&'static str),
    /// The other side said goodbye.
    #[error("the other side closed the session ({0:?})")]
    Closed(ByeReason),
    #[error("the connection ended")]
    Disconnected,
    #[error("network error: {0}")]
    Io(#[from] io::Error),
    #[error("audio codec error: {0}")]
    Codec(String),
}

impl LinkError {
    pub fn name(&self) -> LinkErrorName {
        match self {
            Self::NotPaired | Self::Closed(ByeReason::Unpaired) => LinkErrorName::HelperNotPaired,
            Self::PairingFailed(_) => LinkErrorName::HelperPairingFailed,
            Self::ModelMissing { .. } => LinkErrorName::HelperModelMissing,
            Self::Incompatible(_) => LinkErrorName::HelperIncompatible,
            _ => LinkErrorName::HelperUnreachable,
        }
    }

    /// A read timeout or a would-block: nothing arrived yet, the connection is still fine.
    pub(crate) fn is_idle(error: &io::Error) -> bool {
        matches!(error.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut)
    }
}

impl From<snow::Error> for LinkError {
    fn from(_: snow::Error) -> Self {
        Self::Crypto
    }
}

impl From<postcard::Error> for LinkError {
    fn from(_: postcard::Error) -> Self {
        Self::Protocol("message encoding")
    }
}
