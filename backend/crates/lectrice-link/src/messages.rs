//! Protocol v1 messages. Client = the phone, server = the computer. Encoded with `postcard`; every
//! decoded message is checked against [`crate::limits`] before anyone uses it.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::{LinkError, LinkResult};
use crate::limits::{
    CLIP_CHUNK_BYTES, MAX_ENGINES, MAX_ITEMS_PER_BATCH, MAX_NAME_CHARS, MAX_SAMPLE_RATE, MAX_SHORT_TEXT_BYTES, MAX_TEXT_BYTES, MAX_VOICES, MIN_SAMPLE_RATE,
};

/// Exchanged once inside the pairing channel, both ways.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairHello {
    pub device_name: String,
    pub app_version: String,
    pub proto_minor: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hello {
    pub proto_minor: u16,
    pub app_version: String,
    pub device_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineOffer {
    pub engine_id: String,
    pub revision: String,
    pub installed: bool,
    pub sample_rate: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Welcome {
    pub proto_minor: u16,
    pub app_version: String,
    pub computer_name: String,
    pub engines: Vec<EngineOffer>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Open {
    pub engine_id: String,
    pub revision: String,
    pub voice_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ready {
    pub engine_id: String,
    pub revision: String,
    pub sample_rate: u32,
    pub voices: Vec<String>,
}

/// Why the computer won't make clips for this session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Refusal {
    /// The phone's model isn't installed here; `bundle_id` is the one to download.
    ModelMissing { bundle_id: String },
    /// Same model, another front-end revision.
    RevisionMismatch { revision: String },
    /// Another model is loaded and a second one doesn't fit in memory right now.
    Busy,
}

/// One sentence to make. `item_id` is the sentence id: meaningless without the book, which never leaves the phone.
/// `Debug` prints the text's length only, so no log line can carry book text.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    pub item_id: u32,
    pub voice_id: String,
    pub text: String,
    /// The player needs it now: it goes before every fill item.
    pub urgent: bool,
}

impl fmt::Debug for Item {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Item")
            .field("item_id", &self.item_id)
            .field("voice_id", &self.voice_id)
            .field("text_bytes", &self.text.len())
            .field("urgent", &self.urgent)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Make {
    pub batch_id: u32,
    pub items: Vec<Item>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CancelScope {
    Batch(u32),
    All,
}

/// Why a side closes the session; the other side reacts at once instead of waiting for a timeout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ByeReason {
    Quit,
    HelperOff,
    Sleeping,
    Unpaired,
    EngineSwitch,
    /// The phone has nothing to ask for (paused, or the buffer is full and the governor is off).
    Idle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClientMessage {
    Hello(Hello),
    Open(Open),
    Make(Make),
    Cancel(CancelScope),
    Ping { t_ms: u64 },
    Pong { t_ms: u64 },
    Bye(ByeReason),
}

/// One piece of a clip's FLAC stream. The header fields repeat in every chunk and must not change within a clip.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipChunk {
    pub item_id: u32,
    pub sample_rate: u32,
    /// Samples in the whole clip.
    pub samples: u32,
    pub make_ms: u32,
    pub seq: u16,
    pub last: bool,
    pub flac: Vec<u8>,
}

/// A registry error name and code (e.g. `VoiceUnknown` 2001): the phone then makes that sentence itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemError {
    pub name: String,
    pub code: u32,
}

/// The computer is busy with its own listener or another phone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Load {
    pub queue_items: u32,
    pub own_listener: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServerMessage {
    Welcome(Welcome),
    Ready(Ready),
    Refused(Refusal),
    Clip(ClipChunk),
    ItemFailed { item_id: u32, error: ItemError },
    Load(Load),
    Ping { t_ms: u64 },
    Pong { t_ms: u64 },
    Bye(ByeReason),
}

/// Bounds a message must meet after decoding.
pub(crate) trait Validate {
    fn validate(&self) -> LinkResult<()>;
}

/// Ids, versions and names: short, and without control or direction-changing characters (they reach the other
/// side's log lines and screens).
fn short(text: &str) -> LinkResult<()> {
    if text.len() > MAX_SHORT_TEXT_BYTES {
        return Err(LinkError::LimitExceeded("short text"));
    }
    if text.chars().any(is_disguising) {
        return Err(LinkError::Protocol("control characters in an id"));
    }
    Ok(())
}

/// A device name shown on the other device: at most `MAX_NAME_CHARS` characters, no control or bidi-override
/// characters (they could disguise the name in the other side's UI or logs).
fn name(text: &str) -> LinkResult<()> {
    short(text)?;
    if text.trim().is_empty() || text.chars().count() > MAX_NAME_CHARS || text.chars().any(is_disguising) {
        return Err(LinkError::Protocol("device name"));
    }
    Ok(())
}

fn is_disguising(character: char) -> bool {
    character.is_control() || matches!(character, '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
}

/// Item text goes into the speech engine's C front end: control characters become spaces before it gets there.
pub fn speakable_text(text: &str) -> String {
    text.chars().map(|character| if character.is_control() || is_disguising(character) { ' ' } else { character }).collect()
}

fn sample_rate(rate: u32) -> LinkResult<()> {
    if !(MIN_SAMPLE_RATE..=MAX_SAMPLE_RATE).contains(&rate) {
        return Err(LinkError::LimitExceeded("sample rate"));
    }
    Ok(())
}

fn voices(list: &[String]) -> LinkResult<()> {
    if list.len() > MAX_VOICES {
        return Err(LinkError::LimitExceeded("voices"));
    }
    list.iter().try_for_each(|voice| short(voice))
}

impl Validate for PairHello {
    fn validate(&self) -> LinkResult<()> {
        name(&self.device_name)?;
        short(&self.app_version)
    }
}

impl Validate for ClientMessage {
    fn validate(&self) -> LinkResult<()> {
        match self {
            Self::Hello(hello) => {
                short(&hello.app_version)?;
                name(&hello.device_name)
            }
            Self::Open(open) => {
                short(&open.engine_id)?;
                short(&open.revision)?;
                voices(&open.voice_ids)
            }
            Self::Make(make) => {
                if make.items.is_empty() || make.items.len() > MAX_ITEMS_PER_BATCH {
                    return Err(LinkError::LimitExceeded("items per batch"));
                }
                make.items.iter().try_for_each(|item| {
                    short(&item.voice_id)?;
                    if item.text.is_empty() || item.text.len() > MAX_TEXT_BYTES {
                        return Err(LinkError::LimitExceeded("item text"));
                    }
                    Ok(())
                })
            }
            Self::Cancel(_) | Self::Ping { .. } | Self::Pong { .. } | Self::Bye(_) => Ok(()),
        }
    }
}

impl Validate for ServerMessage {
    fn validate(&self) -> LinkResult<()> {
        match self {
            Self::Welcome(welcome) => {
                short(&welcome.app_version)?;
                name(&welcome.computer_name)?;
                if welcome.engines.len() > MAX_ENGINES {
                    return Err(LinkError::LimitExceeded("engines"));
                }
                welcome.engines.iter().try_for_each(|offer| {
                    short(&offer.engine_id)?;
                    short(&offer.revision)?;
                    sample_rate(offer.sample_rate)
                })
            }
            Self::Ready(ready) => {
                short(&ready.engine_id)?;
                short(&ready.revision)?;
                sample_rate(ready.sample_rate)?;
                voices(&ready.voices)
            }
            Self::Refused(Refusal::ModelMissing { bundle_id }) => short(bundle_id),
            Self::Refused(Refusal::RevisionMismatch { revision }) => short(revision),
            Self::Clip(chunk) if chunk.flac.len() > CLIP_CHUNK_BYTES => Err(LinkError::LimitExceeded("clip chunk")),
            Self::Clip(chunk) => sample_rate(chunk.sample_rate),
            Self::ItemFailed { error, .. } => short(&error.name),
            Self::Refused(Refusal::Busy) | Self::Load(_) | Self::Ping { .. } | Self::Pong { .. } | Self::Bye(_) => Ok(()),
        }
    }
}
