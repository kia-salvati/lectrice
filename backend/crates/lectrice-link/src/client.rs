//! The phone's side (feature `client`): pair with a computer, then sessions that ask it for clips. Blocking, over
//! `std::net` with timeouts; no async runtime. One reader thread per session answers pings, joins clip chunks,
//! decodes and bounds-checks the FLAC, and hands finished clips over as [`ClientEvent`]s.

use std::collections::HashMap;
use std::io::{self, Write};
use std::net::{Shutdown, SocketAddr, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use snow::Builder;
use tracing::{debug, info};

use crate::error::{LinkError, LinkResult};
use crate::flac::decode_clip;
use crate::frame::FrameReader;
use crate::identity::{KEY_BYTES, PeerId, StaticKeys};
use crate::limits::{CLIP_CHUNK_BYTES, PROTOCOL_MINOR, SESSION_PROLOGUE, Timing, max_clip_samples};
use crate::messages::{
    ByeReason, CancelScope, ClientMessage, ClipChunk, Hello, ItemError, Load, Make, Open, PairHello, Ready, Refusal, ServerMessage, Welcome,
};
use crate::noise::{Channel, IK_PARAMS, Opener, params, receive_handshake, send_handshake};
use crate::pairing::{Paired, PairingCode, pair_as_phone};
use crate::socket::{connect, monotonic_ms, receive_by, send};
use crate::sync::lock;

const SERVICE: &str = "link_client";
/// Room for FLAC's headers on top of the PCM bound.
const FLAC_OVERHEAD_BYTES: u64 = 8 * 1024;
const BYTES_PER_SAMPLE: u64 = 2;
/// Answers to `Open` nobody has read yet. The computer answers each `Open` once; more than this unread means it is
/// sending them unasked, and the session ends instead of queueing them.
const MAX_UNREAD_OPEN_REPLIES: usize = 4;
/// `Load` events nobody has read yet; further ones are dropped until the app catches up (only the latest matters).
const MAX_UNREAD_LOADS: usize = 8;

#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub device_name: String,
    pub app_version: String,
    pub timing: Timing,
    /// Tests only: allow a computer on this machine.
    pub allow_loopback: bool,
}

/// Opens the TCP connection to the computer. The phone's shell binds the socket to the Wi-Fi network before it
/// connects (`android_setsocknetwork`), so mobile data or a VPN can never carry the link. The link
/// checks the address rule before asking, and checks that the socket is connected to that address afterwards.
pub trait SocketFactory: Send + Sync {
    fn connect(&self, address: SocketAddr, timeout: Duration) -> io::Result<TcpStream>;
}

/// `TcpStream::connect_timeout` on whatever network the system picks (desktop tests, the same-machine harness).
#[derive(Debug, Clone, Copy, Default)]
pub struct DirectSockets;

impl SocketFactory for DirectSockets {
    fn connect(&self, address: SocketAddr, timeout: Duration) -> io::Result<TcpStream> {
        TcpStream::connect_timeout(&address, timeout)
    }
}

/// Pairs with the computer at `address` using the code its screen shows. The result's `peer` is the computer to pin.
pub fn pair(address: SocketAddr, code: &PairingCode, keys: &StaticKeys, config: &ClientConfig) -> LinkResult<Paired> {
    pair_via(&DirectSockets, address, code, keys, config)
}

/// [`pair`] over a socket from the shell's `sockets`.
pub fn pair_via(sockets: &dyn SocketFactory, address: SocketAddr, code: &PairingCode, keys: &StaticKeys, config: &ClientConfig) -> LinkResult<Paired> {
    let deadline = Instant::now() + config.timing.handshake;
    let stream = connect(sockets, address, config.timing.handshake, config.allow_loopback)?;
    let mut writer = stream.try_clone()?;
    let mut reader = FrameReader::new(stream);
    let hello = PairHello { device_name: config.device_name.clone(), app_version: config.app_version.clone(), proto_minor: PROTOCOL_MINOR };
    let paired = pair_as_phone(&mut reader, &mut writer, code, keys, &hello, deadline);
    let _ = writer.shutdown(Shutdown::Both);
    let paired = paired?;
    info!(service = SERVICE, peer = paired.peer.peer_id.short(), "Paired with a computer");
    Ok(paired)
}

/// What the session's reader thread hands over.
#[derive(Debug, Clone, PartialEq)]
pub enum ClientEvent {
    /// A whole, decoded clip for an item that is still wanted.
    Clip {
        item_id: u32,
        sample_rate: u32,
        samples: Vec<i16>,
        make_ms: u32,
    },
    ItemFailed {
        item_id: u32,
        error: ItemError,
    },
    Load(Load),
    /// The session ended; no more events follow.
    Closed(ClosedReason),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClosedReason {
    Bye(ByeReason),
    /// Nothing arrived for `Timing::dead_after`.
    Silent,
    /// The connection broke or the computer sent something invalid.
    Broken(String),
    /// [`Session::close`] was called.
    Local,
}

/// Replies to `Open` go to the caller of [`Session::open`], everything else to the event queue.
enum Routed {
    Opened(Result<Ready, Refusal>),
}

struct Writer {
    channel: Channel,
    stream: TcpStream,
    last_sent: Instant,
}

impl Writer {
    fn send(&mut self, message: &ClientMessage) -> LinkResult<()> {
        send(&self.channel, &mut self.stream, message)?;
        self.last_sent = Instant::now();
        Ok(())
    }
}

/// What the reader needs to check a clip: the items asked for and not cancelled, with their text length.
#[derive(Default)]
struct Pending {
    items: HashMap<u32, Wanted>,
    sample_rate: Option<u32>,
}

struct Wanted {
    batch_id: u32,
    text_chars: usize,
}

/// One session with a paired computer.
pub struct Session {
    writer: Arc<Mutex<Writer>>,
    pending: Arc<Mutex<Pending>>,
    events: Receiver<ClientEvent>,
    unread_loads: Arc<AtomicUsize>,
    opened: Receiver<Routed>,
    welcome: Welcome,
    closing: Arc<AtomicBool>,
    reader: Option<JoinHandle<()>>,
    computer: PeerId,
}

impl Session {
    /// TCP + Noise `IK` to the pinned computer key, then `Hello` → `Welcome`.
    pub fn connect(address: SocketAddr, keys: &StaticKeys, computer_key: &[u8; KEY_BYTES], config: &ClientConfig) -> LinkResult<Self> {
        Self::connect_via(&DirectSockets, address, keys, computer_key, config)
    }

    /// [`Session::connect`] over a socket from the shell's `sockets`.
    pub fn connect_via(
        sockets: &dyn SocketFactory,
        address: SocketAddr,
        keys: &StaticKeys,
        computer_key: &[u8; KEY_BYTES],
        config: &ClientConfig,
    ) -> LinkResult<Self> {
        let started = Instant::now();
        let deadline = started + config.timing.handshake;
        let mut stream = connect(sockets, address, config.timing.handshake, config.allow_loopback)?;
        stream.write_all(SESSION_PROLOGUE)?;
        let mut reader = FrameReader::new(stream.try_clone()?);
        let mut state = Builder::new(params(IK_PARAMS))
            .local_private_key(keys.private())?
            .remote_public_key(computer_key)?
            .prologue(SESSION_PROLOGUE)?
            .build_initiator()?;
        send_handshake(&mut state, &mut stream, &[])?;
        receive_handshake(&mut state, &mut reader, deadline)?;
        let channel = Channel::new(state.into_transport_mode()?);
        let mut opener = channel.opener();
        let mut writer = Writer { channel, stream, last_sent: Instant::now() };
        writer.send(&ClientMessage::Hello(Hello {
            proto_minor: PROTOCOL_MINOR,
            app_version: config.app_version.clone(),
            device_name: config.device_name.clone(),
        }))?;
        let welcome = match receive_by::<ServerMessage, _>(&mut opener, &mut reader, deadline)? {
            ServerMessage::Welcome(welcome) => welcome,
            ServerMessage::Bye(ByeReason::Unpaired) => return Err(LinkError::NotPaired),
            ServerMessage::Bye(reason) => return Err(LinkError::Closed(reason)),
            _ => return Err(LinkError::Protocol("expected Welcome")),
        };
        let computer = crate::identity::PeerId::of(computer_key);
        info!(service = SERVICE, peer = computer.short(), connect_ms = started.elapsed().as_millis() as u64, "Session open");

        let writer = Arc::new(Mutex::new(writer));
        let pending = Arc::new(Mutex::new(Pending::default()));
        let closing = Arc::new(AtomicBool::new(false));
        // Clips and failures are only queued for items this phone asked for, so their number is the phone's own.
        let (event_sender, events) = mpsc::channel();
        let unread_loads = Arc::new(AtomicUsize::new(0));
        let (opened_sender, opened) = mpsc::sync_channel(MAX_UNREAD_OPEN_REPLIES);
        let loop_state = ReaderLoop {
            reader,
            opener,
            writer: writer.clone(),
            pending: pending.clone(),
            events: event_sender,
            unread_loads: unread_loads.clone(),
            opened: opened_sender,
            closing: closing.clone(),
            timing: config.timing,
            assembling: None,
            last_received: Instant::now(),
        };
        let reader = thread::Builder::new().name("link-client-reader".into()).spawn(move || loop_state.run())?;
        Ok(Self { writer, pending, events, unread_loads, opened, welcome, closing, reader: Some(reader), computer })
    }

    pub fn welcome(&self) -> &Welcome {
        &self.welcome
    }

    pub fn computer(&self) -> &PeerId {
        &self.computer
    }

    /// Asks for an engine; `Ok(Ready)` only when the computer has it, else the refusal as an error.
    pub fn open(&self, open: Open, timeout: Duration) -> LinkResult<Ready> {
        self.send(&ClientMessage::Open(open))?;
        match self.opened.recv_timeout(timeout) {
            Ok(Routed::Opened(Ok(ready))) => Ok(ready),
            Ok(Routed::Opened(Err(Refusal::ModelMissing { bundle_id }))) => Err(LinkError::ModelMissing { bundle_id }),
            Ok(Routed::Opened(Err(Refusal::RevisionMismatch { .. }))) => Err(LinkError::Incompatible("front-end revision")),
            Ok(Routed::Opened(Err(Refusal::Busy))) => Err(LinkError::Closed(ByeReason::Quit)),
            Err(RecvTimeoutError::Timeout) => Err(LinkError::Timeout("open")),
            Err(RecvTimeoutError::Disconnected) => Err(LinkError::Disconnected),
        }
    }

    pub fn make(&self, make: Make) -> LinkResult<()> {
        {
            let mut pending = lock(&self.pending);
            for item in &make.items {
                pending.items.insert(item.item_id, Wanted { batch_id: make.batch_id, text_chars: item.text.chars().count() });
            }
        }
        self.send(&ClientMessage::Make(make))
    }

    /// Clips of cancelled items that still arrive are dropped.
    pub fn cancel(&self, scope: CancelScope) -> LinkResult<()> {
        {
            let mut pending = lock(&self.pending);
            match scope {
                CancelScope::All => pending.items.clear(),
                CancelScope::Batch(batch) => pending.items.retain(|_, wanted| wanted.batch_id != batch),
            }
        }
        self.send(&ClientMessage::Cancel(scope))
    }

    /// Items asked for whose clip (or failure) hasn't arrived.
    pub fn in_flight(&self) -> usize {
        lock(&self.pending).items.len()
    }

    pub fn next_event(&self, timeout: Duration) -> Option<ClientEvent> {
        let event = self.events.recv_timeout(timeout).ok()?;
        if matches!(event, ClientEvent::Load(_)) {
            self.unread_loads.fetch_sub(1, Ordering::SeqCst);
        }
        Some(event)
    }

    /// Says goodbye and ends the session.
    pub fn close(mut self, reason: ByeReason) {
        let _ = self.send(&ClientMessage::Bye(reason));
        self.shut();
    }

    fn send(&self, message: &ClientMessage) -> LinkResult<()> {
        lock(&self.writer).send(message)
    }

    fn shut(&mut self) {
        self.closing.store(true, Ordering::SeqCst);
        let _ = lock(&self.writer).stream.shutdown(Shutdown::Both);
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.shut();
    }
}

struct Assembly {
    header: ClipChunk,
    next_seq: u16,
    flac: Vec<u8>,
    max_samples: u64,
}

struct ReaderLoop {
    reader: FrameReader<TcpStream>,
    opener: Opener,
    writer: Arc<Mutex<Writer>>,
    pending: Arc<Mutex<Pending>>,
    events: Sender<ClientEvent>,
    unread_loads: Arc<AtomicUsize>,
    opened: SyncSender<Routed>,
    closing: Arc<AtomicBool>,
    timing: Timing,
    assembling: Option<Assembly>,
    last_received: Instant,
}

impl ReaderLoop {
    fn run(mut self) {
        let reason = self.pump();
        debug!(service = SERVICE, reason = ?reason, "Session reader ended");
        let _ = lock(&self.writer).stream.shutdown(Shutdown::Both);
        let _ = self.events.send(ClientEvent::Closed(reason));
    }

    fn pump(&mut self) -> ClosedReason {
        loop {
            if self.closing.load(Ordering::SeqCst) {
                return ClosedReason::Local;
            }
            match self.reader.read_frame() {
                Ok(Some(frame)) => {
                    self.last_received = Instant::now();
                    // Also while clips keep arriving: the read never times out then, and a phone that only listens
                    // for longer than `dead_after` would be taken for gone by the computer.
                    if let Err(error) = self.ping_if_quiet() {
                        return ClosedReason::Broken(error.to_string());
                    }
                    let message = match self.opener.open(&frame) {
                        Ok(Some(bytes)) => crate::codec::decode::<ServerMessage>(&bytes),
                        Ok(None) => continue,
                        Err(error) => Err(error),
                    };
                    match message.and_then(|message| self.handle(message)) {
                        Ok(None) => {}
                        Ok(Some(reason)) => return reason,
                        Err(error) => return ClosedReason::Broken(error.to_string()),
                    }
                }
                Ok(None) => {
                    if self.reader.take_progress() {
                        self.last_received = Instant::now();
                    }
                    if self.last_received.elapsed() >= self.timing.dead_after {
                        return ClosedReason::Silent;
                    }
                    if let Err(error) = self.ping_if_quiet() {
                        return ClosedReason::Broken(error.to_string());
                    }
                }
                Err(_) if self.closing.load(Ordering::SeqCst) => return ClosedReason::Local,
                Err(error) => return ClosedReason::Broken(error.to_string()),
            }
        }
    }

    fn ping_if_quiet(&self) -> LinkResult<()> {
        let mut writer = lock(&self.writer);
        if writer.last_sent.elapsed() >= self.timing.ping_every {
            writer.send(&ClientMessage::Ping { t_ms: monotonic_ms() })?;
        }
        Ok(())
    }

    fn handle(&mut self, message: ServerMessage) -> LinkResult<Option<ClosedReason>> {
        match message {
            ServerMessage::Ping { t_ms } => lock(&self.writer).send(&ClientMessage::Pong { t_ms })?,
            ServerMessage::Pong { .. } => {}
            // The rate clips must have is noted here, on the thread that reads them: noted by the caller of `open`
            // it could come after the first clip.
            ServerMessage::Ready(ready) => {
                self.set_sample_rate(Some(ready.sample_rate));
                self.open_reply(Ok(ready))?;
            }
            ServerMessage::Refused(refusal) => {
                self.set_sample_rate(None);
                self.open_reply(Err(refusal))?;
            }
            ServerMessage::Clip(chunk) => self.take_chunk(chunk)?,
            ServerMessage::ItemFailed { item_id, error } => {
                if lock(&self.pending).items.remove(&item_id).is_some() {
                    let _ = self.events.send(ClientEvent::ItemFailed { item_id, error });
                }
            }
            ServerMessage::Load(load) => {
                if self.unread_loads.load(Ordering::SeqCst) < MAX_UNREAD_LOADS {
                    self.unread_loads.fetch_add(1, Ordering::SeqCst);
                    let _ = self.events.send(ClientEvent::Load(load));
                }
            }
            ServerMessage::Bye(reason) => return Ok(Some(ClosedReason::Bye(reason))),
            ServerMessage::Welcome(_) => return Err(LinkError::Protocol("a second Welcome")),
        }
        Ok(None)
    }

    fn set_sample_rate(&self, sample_rate: Option<u32>) {
        lock(&self.pending).sample_rate = sample_rate;
    }

    fn open_reply(&self, reply: Result<Ready, Refusal>) -> LinkResult<()> {
        match self.opened.try_send(Routed::Opened(reply)) {
            Err(TrySendError::Full(_)) => Err(LinkError::LimitExceeded("unasked Open replies")),
            // The session was dropped; the reader ends on its own.
            Ok(()) | Err(TrySendError::Disconnected(_)) => Ok(()),
        }
    }

    /// Clips arrive one at a time, chunks in order; a clip is handed over only after its last chunk decodes within
    /// bounds, so a link cut mid-clip never yields a partial clip.
    fn take_chunk(&mut self, chunk: ClipChunk) -> LinkResult<()> {
        let mut assembly = match self.assembling.take() {
            None => {
                if chunk.seq != 0 {
                    return Err(LinkError::Protocol("clip chunk order"));
                }
                let pending = lock(&self.pending);
                if pending.sample_rate != Some(chunk.sample_rate) {
                    return Err(LinkError::Protocol("clip sample rate"));
                }
                // An item cancelled meanwhile still gets its chunks read, then dropped.
                let text_chars = pending.items.get(&chunk.item_id).map_or(0, |wanted| wanted.text_chars);
                let max_samples = max_clip_samples(text_chars, chunk.sample_rate);
                if u64::from(chunk.samples) > max_samples {
                    return Err(LinkError::LimitExceeded("clip length"));
                }
                Assembly { header: ClipChunk { flac: Vec::new(), ..chunk.clone() }, next_seq: 0, flac: Vec::new(), max_samples }
            }
            Some(assembly) => assembly,
        };
        let header = &assembly.header;
        if chunk.seq != assembly.next_seq || chunk.item_id != header.item_id || chunk.sample_rate != header.sample_rate || chunk.samples != header.samples {
            return Err(LinkError::Protocol("clip chunk order"));
        }
        if (assembly.flac.len() + chunk.flac.len()) as u64 > assembly.max_samples * BYTES_PER_SAMPLE + FLAC_OVERHEAD_BYTES {
            return Err(LinkError::LimitExceeded("clip bytes"));
        }
        debug_assert!(chunk.flac.len() <= CLIP_CHUNK_BYTES);
        assembly.flac.extend_from_slice(&chunk.flac);
        assembly.next_seq = assembly.next_seq.checked_add(1).ok_or(LinkError::LimitExceeded("clip chunks"))?;
        if !chunk.last {
            self.assembling = Some(assembly);
            return Ok(());
        }
        let wanted = lock(&self.pending).items.remove(&chunk.item_id).is_some();
        if !wanted {
            return Ok(());
        }
        let samples = decode_clip(&assembly.flac, assembly.header.sample_rate, assembly.max_samples)?;
        if samples.len() as u64 != u64::from(assembly.header.samples) {
            return Err(LinkError::Protocol("clip sample count"));
        }
        let _ =
            self.events.send(ClientEvent::Clip { item_id: chunk.item_id, sample_rate: assembly.header.sample_rate, samples, make_ms: assembly.header.make_ms });
        Ok(())
    }
}
