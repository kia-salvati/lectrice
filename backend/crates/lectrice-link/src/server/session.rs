//! One phone's session on the computer: the IK handshake, then a reader (this connection's thread) and a writer
//! thread; plus the maker that serves every session's items through the gate.

use std::collections::VecDeque;
use std::net::{IpAddr, Shutdown, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use snow::Builder;
use tracing::{debug, info, warn};

use super::queue::SessionKey;
use super::{ClipEngine, GatePriority, HalfOpen, SERVICE, ServerEvent, SessionEnd, Shared, lock};
use crate::codec;
use crate::error::{LinkError, LinkErrorName, LinkResult};
use crate::flac::{MIN_CLIP_SAMPLES, encode_clip};
use crate::frame::FrameReader;
use crate::identity::{KEY_BYTES, PeerRecord};
use crate::limits::{
    CLIP_BACKLOG_BYTES, CLIP_CHUNK_BYTES, MAX_OUTBOX_BYTES, MAX_QUEUED_ITEMS_PER_PHONE, MAX_SESSIONS, PHONE_ITEMS_PER_MINUTE, PROTOCOL_MINOR, SESSION_PROLOGUE,
    max_clip_samples,
};
use crate::messages::{ByeReason, CancelScope, ClientMessage, ClipChunk, Item, ItemError, Load, Make, Open, Ready, ServerMessage, Welcome, speakable_text};
use crate::noise::{Channel, IK_PARAMS, Opener, params, receive_handshake, send_handshake};
use crate::pcm::pcm16_from_f32;
use crate::socket::{monotonic_ms, receive_by, send};

const VOICE_UNKNOWN: (&str, u32) = ("VoiceUnknown", 2001);
const ENGINE_UNAVAILABLE: (&str, u32) = ("SpeechEngineUnavailable", 2002);
const SYNTHESIS_FAILED: (&str, u32) = ("SpeechSynthesisFailed", 2003);
/// Over the per-phone item budget (security review S15).
const HELPER_BUSY: (&str, u32) = (LinkErrorName::HelperBusy.as_str(), LinkErrorName::HelperBusy.code());
const BUDGET_WINDOW: Duration = Duration::from_secs(60);

fn item_error((name, code): (&str, u32)) -> ItemError {
    ItemError { name: name.into(), code }
}

/// What a waiting message counts for besides its payload.
const MESSAGE_OVERHEAD_BYTES: usize = 128;

/// Roughly what `message` holds in memory while it waits in the outbox.
fn outbox_cost(message: &ServerMessage) -> usize {
    let payload = match message {
        ServerMessage::Clip(chunk) => chunk.flac.len(),
        ServerMessage::Ready(ready) => ready.voices.iter().map(String::len).sum(),
        _ => 0,
    };
    MESSAGE_OVERHEAD_BYTES + payload
}

enum Outgoing {
    /// The message and its [`outbox_cost`].
    Message(ServerMessage, usize),
    Close,
}

pub(crate) struct SessionHandle {
    key: SessionKey,
    pub(crate) peer: PeerRecord,
    outbox: Sender<Outgoing>,
    /// The cost of everything in `outbox` that isn't written yet.
    outbox_bytes: AtomicUsize,
    /// To cut the connection from any thread (the writer may be blocked in a write).
    socket: TcpStream,
    engine: Mutex<Option<Arc<dyn ClipEngine>>>,
    /// (batch, item) being made now.
    making: Mutex<Option<(u32, u32)>>,
    discard_current: AtomicBool,
    closed: AtomicBool,
    closed_by_us: Mutex<Option<ByeReason>>,
    own_listener_sent: AtomicBool,
    recent_items: Mutex<VecDeque<Instant>>,
    last_sent: Mutex<Instant>,
}

impl SessionHandle {
    pub(crate) fn engine_id(&self) -> Option<String> {
        lock(&self.engine).as_ref().map(|engine| engine.engine_id())
    }

    pub(crate) fn is_making(&self) -> bool {
        lock(&self.making).is_some()
    }

    /// Queues `message` for the writer. A phone that sends without reading would grow the queue for as long as it
    /// likes (every ping gets a pong); past `MAX_OUTBOX_BYTES` the session ends instead.
    fn send(&self, message: ServerMessage) {
        let cost = outbox_cost(&message);
        if self.outbox_bytes.fetch_add(cost, Ordering::SeqCst) + cost > MAX_OUTBOX_BYTES {
            self.outbox_bytes.fetch_sub(cost, Ordering::SeqCst);
            if !self.closed.swap(true, Ordering::SeqCst) {
                warn!(service = SERVICE, peer = self.peer.peer_id.short(), "Phone isn't reading; session ended");
            }
            let _ = self.outbox.send(Outgoing::Close);
            let _ = self.socket.shutdown(Shutdown::Both);
            return;
        }
        if self.outbox.send(Outgoing::Message(message, cost)).is_err() {
            self.outbox_bytes.fetch_sub(cost, Ordering::SeqCst);
        }
    }

    /// True while clips already made for this phone wait to be read: making more would only pile them up.
    fn is_backlogged(&self) -> bool {
        self.outbox_bytes.load(Ordering::SeqCst) > CLIP_BACKLOG_BYTES
    }

    /// Says goodbye and ends the session (once).
    pub(crate) fn close(&self, reason: ByeReason) {
        if self.closed.swap(true, Ordering::SeqCst) {
            return;
        }
        *lock(&self.closed_by_us) = Some(reason);
        self.send(ServerMessage::Bye(reason));
        let _ = self.outbox.send(Outgoing::Close);
        // The engine goes with the session at once, so an engine reload never waits on it.
        lock(&self.engine).take();
    }

    /// True when `own` differs from what this phone was last told.
    pub(crate) fn own_listener_changed(&self, own: bool) -> bool {
        self.own_listener_sent.swap(own, Ordering::SeqCst) != own
    }

    pub(crate) fn send_load(&self, queued: usize, own_listener: bool) {
        self.send(ServerMessage::Load(Load { queue_items: u32::try_from(queued).unwrap_or(u32::MAX), own_listener }));
    }

    /// How many of `wanted` new items this phone may still queue this minute.
    fn take_budget(&self, wanted: usize) -> usize {
        let now = Instant::now();
        let mut recent = lock(&self.recent_items);
        while recent.front().is_some_and(|at| now.duration_since(*at) > BUDGET_WINDOW) {
            recent.pop_front();
        }
        let allowed = wanted.min(PHONE_ITEMS_PER_MINUTE.saturating_sub(recent.len()));
        recent.extend(std::iter::repeat_n(now, allowed));
        allowed
    }
}

/// After the session prologue: Noise `IK` (the phone's key must be paired), `Hello` → `Welcome`, then the session.
pub(super) fn serve_session(shared: &Arc<Shared>, stream: TcpStream, from: IpAddr, deadline: Instant, half_open: &mut HalfOpen<'_>) -> LinkResult<()> {
    let mut writer = stream.try_clone()?;
    let mut reader = FrameReader::new(stream.try_clone()?);
    let handshake = (|| -> LinkResult<(Channel, [u8; KEY_BYTES])> {
        let mut state = Builder::new(params(IK_PARAMS)).local_private_key(shared.deps.keys.private())?.prologue(SESSION_PROLOGUE)?.build_responder()?;
        receive_handshake(&mut state, &mut reader, deadline)?;
        let phone_key: [u8; KEY_BYTES] = state.get_remote_static().ok_or(LinkError::Crypto)?.try_into().map_err(|_| LinkError::Crypto)?;
        send_handshake(&mut state, &mut writer, &[])?;
        Ok((Channel::new(state.into_transport_mode()?), phone_key))
    })();
    // Only a failed authentication counts towards the address's back-off; a dropped connection doesn't.
    let (channel, phone_key) = handshake.inspect_err(|error| {
        if matches!(error, LinkError::Crypto) {
            shared.handshake_failed(from);
        }
    })?;
    let mut opener = channel.opener();

    let Some(peer) = shared.deps.peers.find(&phone_key)? else {
        // An encrypted "not paired" so the phone can show "Pair again"; nothing else is offered.
        shared.handshake_failed(from);
        send(&channel, &mut writer, &ServerMessage::Bye(ByeReason::Unpaired))?;
        let _ = writer.shutdown(Shutdown::Both);
        return Err(LinkError::NotPaired);
    };
    let hello = match receive_by::<ClientMessage, _>(&mut opener, &mut reader, deadline)? {
        ClientMessage::Hello(hello) => hello,
        _ => return Err(LinkError::Protocol("expected Hello")),
    };
    let welcome = Welcome {
        proto_minor: PROTOCOL_MINOR,
        app_version: shared.app_version.clone(),
        computer_name: lock(&shared.computer_name).clone(),
        engines: shared.deps.engines.offers(),
    };
    send(&channel, &mut writer, &ServerMessage::Welcome(welcome))?;
    shared.handshake_succeeded(from);
    half_open.finish();

    let (outbox, outgoing) = mpsc::channel();
    let key = shared.next_key.fetch_add(1, Ordering::SeqCst);
    let handle = Arc::new(SessionHandle {
        key,
        peer: peer.clone(),
        outbox,
        outbox_bytes: AtomicUsize::new(0),
        socket: stream.try_clone()?,
        engine: Mutex::new(None),
        making: Mutex::new(None),
        discard_current: AtomicBool::new(false),
        closed: AtomicBool::new(false),
        closed_by_us: Mutex::new(None),
        own_listener_sent: AtomicBool::new(false),
        recent_items: Mutex::new(VecDeque::new()),
        last_sent: Mutex::new(Instant::now()),
    });
    {
        let mut sessions = lock(&shared.sessions);
        // A phone that reconnects replaces its old session.
        let old: Vec<_> = sessions.values().filter(|session| session.peer.peer_id == peer.peer_id).cloned().collect();
        old.iter().for_each(|session| session.close(ByeReason::Quit));
        if sessions.len() - old.len() >= MAX_SESSIONS {
            drop(sessions);
            send(&channel, &mut writer, &ServerMessage::Bye(ByeReason::Quit))?;
            return Err(LinkError::LimitExceeded("sessions"));
        }
        sessions.insert(key, handle.clone());
    }
    info!(service = SERVICE, peer = peer.peer_id.short(), proto_minor = hello.proto_minor, "Phone session started");
    shared.notify(ServerEvent::SessionStarted { peer: peer.clone(), app_version: hello.app_version.clone() });

    let writer_thread = {
        let handle = handle.clone();
        let channel = channel.clone();
        thread::Builder::new().name("link-writer".into()).spawn(move || write_loop(&handle, &channel, writer, &outgoing))
    };
    let (end, spawn_error) = match writer_thread {
        Ok(writer_thread) => {
            let reader = SessionReader { shared, handle: &handle, opener, reader, phone_app_version: hello.app_version, last_received: Instant::now() };
            let end = reader.run();
            handle.closed.store(true, Ordering::SeqCst);
            let _ = handle.outbox.send(Outgoing::Close);
            let _ = writer_thread.join();
            (end, None)
        }
        // Without a writer the session can't run; it still has to leave the session list below.
        Err(error) => {
            handle.closed.store(true, Ordering::SeqCst);
            (SessionEnd::Broken, Some(error))
        }
    };
    let _ = stream.shutdown(Shutdown::Both);
    lock(&handle.engine).take();

    let removed = {
        let mut sessions = lock(&shared.sessions);
        let removed = sessions.get(&key).is_some_and(|session| Arc::ptr_eq(session, &handle));
        if removed {
            sessions.remove(&key);
        }
        removed
    };
    lock(&shared.queue).remove_session(key);
    let end = lock(&handle.closed_by_us).map_or(end, SessionEnd::Closed);
    info!(service = SERVICE, peer = peer.peer_id.short(), end = ?end, "Phone session ended");
    if removed {
        shared.notify(ServerEvent::SessionEnded { peer_id: peer.peer_id, end });
    }
    spawn_error.map_or(Ok(()), |error| Err(error.into()))
}

fn write_loop(handle: &SessionHandle, channel: &Channel, mut writer: TcpStream, outgoing: &Receiver<Outgoing>) {
    while let Ok(next) = outgoing.recv() {
        match next {
            Outgoing::Message(message, cost) => {
                let sent = send(channel, &mut writer, &message);
                handle.outbox_bytes.fetch_sub(cost, Ordering::SeqCst);
                if let Err(error) = sent {
                    debug!(service = SERVICE, error = %error, "Session write failed");
                    break;
                }
                *lock(&handle.last_sent) = Instant::now();
            }
            Outgoing::Close => break,
        }
    }
    handle.closed.store(true, Ordering::SeqCst);
    let _ = writer.shutdown(Shutdown::Both);
}

struct SessionReader<'a> {
    shared: &'a Arc<Shared>,
    handle: &'a Arc<SessionHandle>,
    opener: Opener,
    reader: FrameReader<TcpStream>,
    phone_app_version: String,
    last_received: Instant,
}

impl SessionReader<'_> {
    fn run(mut self) -> SessionEnd {
        loop {
            if self.handle.closed.load(Ordering::SeqCst) {
                return SessionEnd::Broken;
            }
            match self.reader.read_frame() {
                Ok(Some(frame)) => {
                    self.last_received = Instant::now();
                    let message = match self.opener.open(&frame) {
                        Ok(Some(bytes)) => codec::decode::<ClientMessage>(&bytes),
                        Ok(None) => continue,
                        Err(error) => Err(error),
                    };
                    match message {
                        Ok(ClientMessage::Bye(reason)) => return SessionEnd::Bye(reason),
                        Ok(message) => {
                            if let Err(error) = self.handle_message(message) {
                                debug!(service = SERVICE, error = %error, "Session message refused");
                                return SessionEnd::Broken;
                            }
                        }
                        Err(error) => {
                            debug!(service = SERVICE, error = %error, "Session frame refused");
                            return SessionEnd::Broken;
                        }
                    }
                }
                Ok(None) => {
                    if self.reader.take_progress() {
                        self.last_received = Instant::now();
                    }
                    if self.last_received.elapsed() >= self.shared.timing.dead_after {
                        return SessionEnd::Silent;
                    }
                    if lock(&self.handle.last_sent).elapsed() >= self.shared.timing.ping_every {
                        self.handle.send(ServerMessage::Ping { t_ms: monotonic_ms() });
                    }
                }
                Err(_) => return SessionEnd::Broken,
            }
        }
    }

    fn handle_message(&mut self, message: ClientMessage) -> LinkResult<()> {
        match message {
            ClientMessage::Hello(_) => return Err(LinkError::Protocol("a second Hello")),
            ClientMessage::Open(open) => self.open(open),
            ClientMessage::Make(make) => self.make(make),
            ClientMessage::Cancel(scope) => self.cancel(scope),
            ClientMessage::Ping { t_ms } => self.handle.send(ServerMessage::Pong { t_ms }),
            ClientMessage::Pong { .. } | ClientMessage::Bye(_) => {}
        }
        Ok(())
    }

    fn open(&self, open: Open) {
        // A new Open replaces the engine; whatever was queued for the old one is dropped.
        self.cancel(CancelScope::All);
        lock(&self.handle.engine).take();
        match self.shared.deps.engines.open(&open.engine_id, &open.revision) {
            Ok(engine) => {
                let ready = Ready { engine_id: engine.engine_id(), revision: engine.revision(), sample_rate: engine.sample_rate(), voices: engine.voices() };
                info!(service = SERVICE, peer = self.handle.peer.peer_id.short(), engine_id = %ready.engine_id, "Engine opened for a phone");
                *lock(&self.handle.engine) = Some(engine);
                self.handle.send(ServerMessage::Ready(ready));
            }
            Err(refusal) => {
                info!(service = SERVICE, peer = self.handle.peer.peer_id.short(), engine_id = %open.engine_id, refusal = ?refusal, "Engine refused for a phone");
                self.handle.send(ServerMessage::Refused(refusal.clone()));
                self.shared.notify(ServerEvent::Refused {
                    peer_id: self.handle.peer.peer_id.clone(),
                    phone_app_version: self.phone_app_version.clone(),
                    refusal,
                });
            }
        }
    }

    fn make(&self, make: Make) {
        let Make { batch_id, items } = make;
        let voices = lock(&self.handle.engine).as_ref().map(|engine| engine.voices());
        let Some(voices) = voices else {
            items.iter().for_each(|item| self.fail(item.item_id, ENGINE_UNAVAILABLE));
            return;
        };
        let (known, unknown): (Vec<Item>, Vec<Item>) = items.into_iter().partition(|item| voices.contains(&item.voice_id));
        unknown.iter().for_each(|item| self.fail(item.item_id, VOICE_UNKNOWN));
        let room = MAX_QUEUED_ITEMS_PER_PHONE.saturating_sub(lock(&self.shared.queue).len_of(self.handle.key));
        let allowed = self.handle.take_budget(known.len().min(room));
        if allowed < known.len() {
            warn!(service = SERVICE, peer = self.handle.peer.peer_id.short(), refused = known.len() - allowed, room, "Phone over its item budget");
        }
        known[allowed..].iter().for_each(|item| self.fail(item.item_id, HELPER_BUSY));
        let accepted: Vec<Item> = known.into_iter().take(allowed).collect();
        if accepted.is_empty() {
            return;
        }
        let queued = {
            let mut queue = lock(&self.shared.queue);
            queue.push(self.handle.key, batch_id, accepted);
            queue.len_of(self.handle.key)
        };
        self.shared.work.notify_all();
        self.shared.notify(ServerEvent::Activity { peer_id: self.handle.peer.peer_id.clone(), queued, making: self.handle.is_making() });
    }

    fn cancel(&self, scope: CancelScope) {
        let removed = lock(&self.shared.queue).cancel(self.handle.key, scope);
        if let Some((batch, _)) = *lock(&self.handle.making)
            && (scope == CancelScope::All || scope == CancelScope::Batch(batch))
        {
            self.handle.discard_current.store(true, Ordering::SeqCst);
        }
        debug!(service = SERVICE, peer = self.handle.peer.peer_id.short(), removed, "Phone items cancelled");
    }

    fn fail(&self, item_id: u32, error: (&str, u32)) {
        self.handle.send(ServerMessage::ItemFailed { item_id, error: item_error(error) });
    }
}

/// One item at a time for all phones, through the gate.
pub(super) fn maker_loop(shared: &Arc<Shared>) {
    while !shared.stopping.load(Ordering::SeqCst) {
        let next = {
            let mut queue = lock(&shared.queue);
            loop {
                if shared.stopping.load(Ordering::SeqCst) {
                    return;
                }
                if let Some(next) = queue.pop() {
                    break next;
                }
                queue = shared.work.wait_timeout(queue, crate::socket::POLL).unwrap_or_else(std::sync::PoisonError::into_inner).0;
            }
        };
        let Some(session) = lock(&shared.sessions).get(&next.session).cloned() else { continue };
        if session.closed.load(Ordering::SeqCst) {
            continue;
        }
        let Some(engine) = lock(&session.engine).clone() else {
            session.send(ServerMessage::ItemFailed { item_id: next.item.item_id, error: item_error(ENGINE_UNAVAILABLE) });
            continue;
        };
        if session.is_backlogged() {
            debug!(service = SERVICE, peer = session.peer.peer_id.short(), item_id = next.item.item_id, "Item refused: the phone is behind on reading clips");
            session.send(ServerMessage::ItemFailed { item_id: next.item.item_id, error: item_error(HELPER_BUSY) });
            continue;
        }
        *lock(&session.making) = Some((next.batch_id, next.item.item_id));
        session.discard_current.store(false, Ordering::SeqCst);
        notify_activity(shared, &session);

        let priority = if next.urgent { GatePriority::PhoneUrgent } else { GatePriority::PhoneFill };
        let text = speakable_text(&next.item.text);
        let started = Instant::now();
        let result = {
            let _turn = shared.deps.gate.acquire(priority);
            engine.synthesize(&text, &next.item.voice_id)
        };
        let make_ms = u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX);
        drop(engine);
        *lock(&session.making) = None;

        if session.discard_current.load(Ordering::SeqCst) || session.closed.load(Ordering::SeqCst) {
            debug!(service = SERVICE, item_id = next.item.item_id, "Made item discarded (cancelled)");
        } else {
            match result.and_then(|samples| clip_messages(next.item.item_id, &next.item.text, &samples, sample_rate_of(&session), make_ms)) {
                Ok(messages) => {
                    debug!(
                        service = SERVICE,
                        peer = session.peer.peer_id.short(),
                        item_id = next.item.item_id,
                        make_ms,
                        urgent = next.urgent,
                        "Clip made for a phone"
                    );
                    messages.into_iter().for_each(|message| session.send(message));
                }
                Err(error) => {
                    warn!(service = SERVICE, item_id = next.item.item_id, error = %error.name, "Clip failed for a phone");
                    session.send(ServerMessage::ItemFailed { item_id: next.item.item_id, error });
                }
            }
        }
        notify_activity(shared, &session);
    }
}

fn sample_rate_of(session: &SessionHandle) -> Option<u32> {
    lock(&session.engine).as_ref().map(|engine| engine.sample_rate())
}

fn notify_activity(shared: &Shared, session: &SessionHandle) {
    let queued = lock(&shared.queue).len_of(session.key);
    shared.notify(ServerEvent::Activity { peer_id: session.peer.peer_id.clone(), queued, making: session.is_making() });
}

/// PCM16 (the WAV encoder's rule), padded to one FLAC block, bounded like the phone bounds it, then FLAC in chunks.
fn clip_messages(item_id: u32, text: &str, samples: &[f32], sample_rate: Option<u32>, make_ms: u32) -> Result<Vec<ServerMessage>, ItemError> {
    let sample_rate = sample_rate.ok_or_else(|| item_error(ENGINE_UNAVAILABLE))?;
    let mut pcm: Vec<i16> = samples.iter().map(|&sample| pcm16_from_f32(sample)).collect();
    if pcm.len() < MIN_CLIP_SAMPLES {
        pcm.resize(MIN_CLIP_SAMPLES, 0);
    }
    if pcm.len() as u64 > max_clip_samples(text.chars().count(), sample_rate) {
        return Err(item_error(SYNTHESIS_FAILED));
    }
    let flac = encode_clip(&pcm, sample_rate).map_err(|_| item_error(SYNTHESIS_FAILED))?;
    let total = u32::try_from(pcm.len()).map_err(|_| item_error(SYNTHESIS_FAILED))?;
    let chunks: Vec<&[u8]> = flac.chunks(CLIP_CHUNK_BYTES).collect();
    let count = chunks.len();
    chunks
        .into_iter()
        .enumerate()
        .map(|(seq, bytes)| {
            Ok(ServerMessage::Clip(ClipChunk {
                item_id,
                sample_rate,
                samples: total,
                make_ms,
                seq: u16::try_from(seq).map_err(|_| item_error(SYNTHESIS_FAILED))?,
                last: seq + 1 == count,
                flac: bytes.to_vec(),
            }))
        })
        .collect()
}
