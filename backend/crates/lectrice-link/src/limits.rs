//! Every bound and timing of protocol v1 (file 24 §3.2, §3.5, §3.9). Both sides enforce the bounds on what they read.

use std::time::Duration;

/// A different major refuses the connection; minors only add optional fields.
pub const PROTOCOL_MAJOR: u16 = 1;
pub const PROTOCOL_MINOR: u16 = 0;

/// The only plaintext on the wire before the handshakes; both handshakes hash it, so it can't be downgraded.
pub const PAIR_PROLOGUE: &[u8] = b"lectrice-link/1 pair\n";
pub const SESSION_PROLOGUE: &[u8] = b"lectrice-link/1 session\n";
pub const MAX_PROLOGUE_BYTES: usize = 32;

/// The service type the computer advertises with mDNS / DNS-SD.
pub const SERVICE_TYPE: &str = "_lectrice._tcp";
pub const DEFAULT_PORT: u16 = 47878;

/// A Noise message (and so a frame) can't be longer.
pub const MAX_FRAME_BYTES: usize = 65_535;
/// A message after its fragments are joined.
pub const MAX_MESSAGE_BYTES: usize = 2 * 1024 * 1024;
/// FLAC bytes per `Clip` chunk.
pub const CLIP_CHUNK_BYTES: usize = 60_000;

pub const MAX_ITEMS_PER_BATCH: usize = 32;
pub const MAX_TEXT_BYTES: usize = 4 * 1024;
/// Device names, app versions, engine ids, revisions, voice ids, error names.
pub const MAX_SHORT_TEXT_BYTES: usize = 128;
/// Device and computer names, in characters.
pub const MAX_NAME_CHARS: usize = 40;
pub const MAX_VOICES: usize = 256;
pub const MAX_ENGINES: usize = 16;

/// A clip is never longer than this, and never longer than `max(MIN_CLIP_BOUND_SECONDS, chars ÷ CHARS_PER_SECOND_BOUND)`.
pub const MAX_CLIP_SECONDS: u32 = 120;
pub const MIN_CLIP_BOUND_SECONDS: u32 = 10;
pub const CHARS_PER_SECOND_BOUND: u32 = 6;
/// Sample rates a speech engine can have. Every clip bound is `seconds × rate`, so a rate the other side made up
/// (up to 4 GHz fits the field) would lift them all.
pub const MIN_SAMPLE_RATE: u32 = 8_000;
pub const MAX_SAMPLE_RATE: u32 = 48_000;

pub const PAIR_CODE_DIGITS: usize = 6;
pub const PAIR_CODE_LIFETIME: Duration = Duration::from_secs(120);
/// Wrong codes per pairing window, counted across every address; then the window closes and only the computer's user
/// can open a new one (security review S4: a new code per failure would not lower an attacker's odds).
pub const PAIR_MAX_FAILURES: u32 = 3;
/// Wrong codes per hour across all windows; beyond that pairing waits `PAIR_COOLDOWN`.
pub const PAIR_FAILURES_PER_HOUR: usize = 10;
pub const PAIR_FAILURE_WINDOW: Duration = Duration::from_secs(3600);
pub const PAIR_COOLDOWN: Duration = Duration::from_secs(600);
/// A pairing attempt that makes no progress for this long is dropped (it learned nothing, so it isn't a wrong code).
pub const PAIR_STEP_TIMEOUT: Duration = Duration::from_secs(2);
/// Pairing attempts at the same time, so a staller can't hold the only slot; one per address, so one machine needs
/// several addresses to hold them all.
pub const MAX_PARALLEL_PAIRINGS: usize = 3;
pub const SPAKE2_ID_PHONE: &[u8] = b"lectrice-phone";
pub const SPAKE2_ID_COMPUTER: &[u8] = b"lectrice-desktop";
pub const PAIR_PSK_INFO: &[u8] = b"lectrice-pair-v1";

pub const MAX_PAIRED_PHONES: usize = 8;
pub const MAX_PAIRED_COMPUTERS: usize = 4;

/// A paired phone's budget, so it can't starve the computer's own read-ahead and exports (security review S15): after
/// this many urgent items in a row from one phone, its next urgent item waits like a fill item; and at most this many
/// items per minute per phone (more are answered `ItemFailed` / `HelperBusy`).
pub const PHONE_URGENT_IN_A_ROW: u32 = 4;
pub const PHONE_ITEMS_PER_MINUTE: usize = 240;
/// What one phone may have waiting on the computer (security review pass 3). The phone's own flow control keeps a
/// dozen items in flight; a phone that ignores it gets `HelperBusy` for the items past this many waiting ones.
pub const MAX_QUEUED_ITEMS_PER_PHONE: usize = 256;
/// Made clips waiting for a slow phone to read them: past this many bytes the computer stops making that phone's
/// items (`HelperBusy`) until it has caught up.
pub const CLIP_BACKLOG_BYTES: usize = 8 * 1024 * 1024;
/// Everything waiting to be written to one phone (clips, pongs, failures): past this the phone isn't reading and
/// the session ends, so a phone that only sends can't grow the computer's memory.
pub const MAX_OUTBOX_BYTES: usize = 16 * 1024 * 1024;

/// The server's guards against floods (file 24 §14). Session (IK) failures are counted per source address and never
/// lock out pairing or other phones.
pub const MAX_HALF_OPEN_CONNECTIONS: usize = 16;
/// Handshakes under way from one address, so one machine can't hold every half-open slot.
pub const MAX_HALF_OPEN_PER_ADDRESS: usize = 4;
pub const MAX_SESSIONS: usize = 8;
pub const FAILED_HANDSHAKES_BEFORE_BACKOFF: u32 = 3;
pub const HANDSHAKE_BACKOFF: Duration = Duration::from_secs(60);
/// Addresses the back-off remembers; the oldest entry goes first (a machine can take many addresses).
pub const MAX_BACKOFF_ADDRESSES: usize = 1024;

/// The phone's waits between reconnect attempts (file 24 §3.9): 1, 2, 5, 10, 30 s, then every 60 s.
pub const RECONNECT_DELAYS: [Duration; 6] =
    [Duration::from_secs(1), Duration::from_secs(2), Duration::from_secs(5), Duration::from_secs(10), Duration::from_secs(30), Duration::from_secs(60)];

/// How long to wait after `failures` attempts in a row failed (the first retry follows the first failure).
pub fn reconnect_delay(failures: u32) -> Duration {
    let index = usize::try_from(failures.saturating_sub(1)).unwrap_or(usize::MAX).min(RECONNECT_DELAYS.len() - 1);
    RECONNECT_DELAYS[index]
}

/// Timings both sides use; tests shorten them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timing {
    /// TCP connect plus the whole handshake.
    pub handshake: Duration,
    /// A ping goes out after this long without sending anything.
    pub ping_every: Duration,
    /// The link counts as lost after this long without receiving anything (3 missed pongs).
    pub dead_after: Duration,
    /// A write that can't progress for this long ends the session.
    pub write_stall: Duration,
}

impl Default for Timing {
    fn default() -> Self {
        Self {
            handshake: Duration::from_secs(5),
            ping_every: Duration::from_secs(5),
            dead_after: Duration::from_secs(15),
            write_stall: Duration::from_secs(15),
        }
    }
}

/// The longest clip, in samples, the phone accepts for a text of `text_chars` characters.
pub fn max_clip_samples(text_chars: usize, sample_rate: u32) -> u64 {
    let by_text = u32::try_from(text_chars).unwrap_or(u32::MAX) / CHARS_PER_SECOND_BOUND;
    let seconds = by_text.clamp(MIN_CLIP_BOUND_SECONDS, MAX_CLIP_SECONDS);
    u64::from(seconds) * u64::from(sample_rate)
}
