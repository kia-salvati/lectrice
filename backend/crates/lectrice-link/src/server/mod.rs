//! The computer's side (feature `server`): accepts paired phones on the listeners the
//! shell opened, makes their items one at a time through the shared [`CpuGate`], and sends the clips back. Blocking
//! threads over `std::net`: an accept loop (also the housekeeping tick), one maker, and a reader + a writer per
//! session. The shell plugs in its engines ([`EngineProvider`]), paired-phone store ([`PeerStore`]) and an observer.
//!
//! The listeners are the shell's choice: bind only the addresses of the network the user turned
//! the helper on for, never `0.0.0.0`.

mod gate;
mod pairing_window;
mod queue;
mod session;

use std::collections::HashMap;
use std::io::Read;
use std::net::{IpAddr, Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use tracing::{debug, info, warn};

pub use gate::{CpuGate, GateGuard, GatePriority};
pub use pairing_window::PairingStatus;

use crate::error::{LinkError, LinkResult, PairingFailure};
use crate::frame::FrameReader;
use crate::identity::{PeerId, PeerRecord, PeerStore, StaticKeys};
use crate::limits::{
    FAILED_HANDSHAKES_BEFORE_BACKOFF, HANDSHAKE_BACKOFF, MAX_BACKOFF_ADDRESSES, MAX_HALF_OPEN_CONNECTIONS, MAX_HALF_OPEN_PER_ADDRESS, MAX_PROLOGUE_BYTES,
    PAIR_PROLOGUE, PAIR_STEP_TIMEOUT, PROTOCOL_MINOR, SESSION_PROLOGUE, Timing,
};
use crate::messages::{ByeReason, EngineOffer, ItemError, PairHello, Refusal};
use crate::pairing::{Paired, pair_as_computer};
use crate::socket::{POLL, check_peer, prepare};
use crate::sync::lock;
use pairing_window::{AttemptEnd, PairingWindow};
use queue::{Queue, SessionKey};
use session::SessionHandle;

const SERVICE: &str = "link_server";
/// The computer's own speech within this window counts as "its own listener is busy" (`Load`).
const OWN_LISTENER_WINDOW: Duration = Duration::from_secs(10);
/// How long `shutdown` waits for the sessions to end.
const SHUTDOWN_WAIT: Duration = Duration::from_secs(3);

/// A loaded engine the phones may use. Text arrives already cleaned of control characters.
pub trait ClipEngine: Send + Sync {
    fn engine_id(&self) -> String;
    /// What changes the sound for the same model (the front end); must equal the phone's for clips to be used.
    fn revision(&self) -> String;
    fn sample_rate(&self) -> u32;
    fn voices(&self) -> Vec<String>;
    /// Mono samples in [-1, 1]; an error names the registry entry (e.g. `SpeechSynthesisFailed` 2003).
    fn synthesize(&self, text: &str, voice_id: &str) -> Result<Vec<f32>, ItemError>;
}

/// The shell's engines: which models it could serve, and one to use for a session.
pub trait EngineProvider: Send + Sync {
    fn offers(&self) -> Vec<EngineOffer>;
    fn open(&self, engine_id: &str, revision: &str) -> Result<Arc<dyn ClipEngine>, Refusal>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionEnd {
    Bye(ByeReason),
    Silent,
    Broken,
    /// This computer closed it (helper off, quit, forget, engine switch).
    Closed(ByeReason),
}

/// What the shell's UI and logs follow. Never carries sentence text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerEvent {
    Pairing(PairingStatus),
    SessionStarted {
        peer: PeerRecord,
        app_version: String,
    },
    SessionEnded {
        peer_id: PeerId,
        end: SessionEnd,
    },
    /// Items waiting for this phone, and whether one is being made now.
    Activity {
        peer_id: PeerId,
        queued: usize,
        making: bool,
    },
    /// The phone asked for a model this computer can't serve.
    Refused {
        peer_id: PeerId,
        phone_app_version: String,
        refusal: Refusal,
    },
}

pub trait ServerObserver: Send + Sync {
    fn changed(&self, event: ServerEvent);
}

#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Shown on the phone; at most 40 characters, no control characters.
    pub computer_name: String,
    pub app_version: String,
    pub timing: Timing,
    /// Tests only: accept phones on this machine.
    pub allow_loopback: bool,
}

pub struct ServerDeps {
    pub keys: StaticKeys,
    pub peers: Arc<dyn PeerStore>,
    pub engines: Arc<dyn EngineProvider>,
    pub gate: Arc<CpuGate>,
    pub observer: Arc<dyn ServerObserver>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionInfo {
    pub peer_id: PeerId,
    pub name: String,
    pub queued: usize,
    pub making: bool,
}

struct Backoff {
    failures: u32,
    last_failure: Instant,
    blocked_until: Option<Instant>,
}

impl Backoff {
    /// Nothing to remember any more: the block ran out, or the failures are old.
    fn is_over(&self, now: Instant) -> bool {
        match self.blocked_until {
            Some(until) => now >= until,
            None => now.duration_since(self.last_failure) >= HANDSHAKE_BACKOFF,
        }
    }
}

struct Shared {
    deps: ServerDeps,
    computer_name: Mutex<String>,
    app_version: String,
    timing: Timing,
    allow_loopback: bool,
    stopping: AtomicBool,
    pairing: Mutex<PairingWindow>,
    sessions: Mutex<HashMap<SessionKey, Arc<SessionHandle>>>,
    next_key: AtomicU64,
    queue: Mutex<Queue>,
    work: Condvar,
    /// Handshakes under way, per source address.
    half_open: Mutex<HashMap<IpAddr, usize>>,
    backoff: Mutex<HashMap<IpAddr, Backoff>>,
}

pub struct Server {
    shared: Arc<Shared>,
    addresses: Vec<SocketAddr>,
    threads: Vec<JoinHandle<()>>,
}

impl Server {
    /// Starts serving on `listeners` (already bound by the shell to the allowed network's addresses).
    pub fn start(listeners: Vec<TcpListener>, deps: ServerDeps, config: ServerConfig) -> LinkResult<Self> {
        let addresses = listeners.iter().map(TcpListener::local_addr).collect::<Result<Vec<_>, _>>()?;
        for listener in &listeners {
            listener.set_nonblocking(true)?;
        }
        let shared = Arc::new(Shared {
            deps,
            computer_name: Mutex::new(config.computer_name),
            app_version: config.app_version,
            timing: config.timing,
            allow_loopback: config.allow_loopback,
            stopping: AtomicBool::new(false),
            pairing: Mutex::new(PairingWindow::new()),
            sessions: Mutex::new(HashMap::new()),
            next_key: AtomicU64::new(1),
            queue: Mutex::new(Queue::default()),
            work: Condvar::new(),
            half_open: Mutex::new(HashMap::new()),
            backoff: Mutex::new(HashMap::new()),
        });
        let accept = {
            let shared = shared.clone();
            thread::Builder::new().name("link-accept".into()).spawn(move || accept_loop(&shared, &listeners))?
        };
        let maker = {
            let shared = shared.clone();
            thread::Builder::new().name("link-maker".into()).spawn(move || session::maker_loop(&shared))?
        };
        info!(service = SERVICE, listeners = addresses.len(), peer = shared.deps.keys.peer_id().short(), "Helper server started");
        Ok(Self { shared, addresses, threads: vec![accept, maker] })
    }

    pub fn local_addresses(&self) -> &[SocketAddr] {
        &self.addresses
    }

    /// Opens the pairing window ("Pair a phone"), or reports why it can't open.
    pub fn start_pairing(&self) -> LinkResult<PairingStatus> {
        let status = lock(&self.shared.pairing).open(Instant::now())?;
        info!(service = SERVICE, opened = matches!(status, PairingStatus::Showing { .. }), "Pairing window requested");
        self.shared.notify(ServerEvent::Pairing(status.clone()));
        Ok(status)
    }

    pub fn cancel_pairing(&self) {
        lock(&self.shared.pairing).close();
        self.shared.notify(ServerEvent::Pairing(PairingStatus::Closed));
    }

    pub fn pairing_status(&self) -> PairingStatus {
        lock(&self.shared.pairing).status(Instant::now())
    }

    /// Used from the next `Welcome` and pairing on.
    pub fn set_computer_name(&self, name: String) {
        *lock(&self.shared.computer_name) = name;
    }

    /// Removes the phone and ends its session with `Bye(Unpaired)`.
    pub fn forget(&self, peer_id: &PeerId) -> LinkResult<bool> {
        let removed = self.shared.deps.peers.remove(peer_id)?;
        self.shared.close_where(|session| session.peer.peer_id == *peer_id, ByeReason::Unpaired);
        info!(service = SERVICE, peer = peer_id.short(), removed, "Phone forgotten");
        Ok(removed)
    }

    /// Before the shell reloads an engine: the sessions using it end (`Bye(engine_switch)`) and let go of it.
    pub fn close_sessions_using(&self, engine_id: &str, reason: ByeReason) {
        self.shared.close_where(|session| session.engine_id().as_deref() == Some(engine_id), reason);
    }

    pub fn sessions(&self) -> Vec<SessionInfo> {
        let queue = lock(&self.shared.queue);
        let mut sessions: Vec<SessionInfo> = lock(&self.shared.sessions)
            .iter()
            .map(|(key, session)| SessionInfo {
                peer_id: session.peer.peer_id.clone(),
                name: session.peer.name.clone(),
                queued: queue.len_of(*key),
                making: session.is_making(),
            })
            .collect();
        sessions.sort_by(|a, b| a.peer_id.cmp(&b.peer_id));
        sessions
    }

    /// Says `Bye(reason)` to every phone, stops listening and waits (briefly) for the sessions to end.
    pub fn shutdown(mut self, reason: ByeReason) {
        self.stop(reason);
    }

    fn stop(&mut self, reason: ByeReason) {
        if self.shared.stopping.swap(true, Ordering::SeqCst) {
            return;
        }
        self.shared.close_where(|_| true, reason);
        self.shared.work.notify_all();
        for thread in self.threads.drain(..) {
            let _ = thread.join();
        }
        let deadline = Instant::now() + SHUTDOWN_WAIT;
        while !lock(&self.shared.sessions).is_empty() && Instant::now() < deadline {
            thread::sleep(POLL / 4);
        }
        info!(service = SERVICE, reason = ?reason, "Helper server stopped");
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop(ByeReason::Quit);
    }
}

impl Shared {
    fn notify(&self, event: ServerEvent) {
        self.deps.observer.changed(event);
    }

    fn close_where(&self, mut matches: impl FnMut(&SessionHandle) -> bool, reason: ByeReason) {
        let chosen: Vec<Arc<SessionHandle>> = lock(&self.sessions).values().filter(|session| matches(session)).cloned().collect();
        for session in chosen {
            session.close(reason);
        }
    }

    fn pairing_changed(&self) {
        let status = lock(&self.pairing).status(Instant::now());
        self.notify(ServerEvent::Pairing(status));
    }

    fn blocked(&self, address: IpAddr) -> bool {
        let mut backoff = lock(&self.backoff);
        let Some(entry) = backoff.get(&address) else { return false };
        if entry.is_over(Instant::now()) {
            backoff.remove(&address);
            return false;
        }
        entry.blocked_until.is_some()
    }

    /// A failed session handshake from `address` (per address only: it never blocks pairing or other phones).
    /// The list is bounded: entries that are over go first, then the oldest, so a machine that fails from many
    /// addresses can't grow it.
    fn handshake_failed(&self, address: IpAddr) {
        let now = Instant::now();
        let mut backoff = lock(&self.backoff);
        if !backoff.contains_key(&address) && backoff.len() >= MAX_BACKOFF_ADDRESSES {
            backoff.retain(|_, entry| !entry.is_over(now));
            if backoff.len() >= MAX_BACKOFF_ADDRESSES
                && let Some(oldest) = backoff.iter().min_by_key(|(_, entry)| entry.last_failure).map(|(address, _)| *address)
            {
                backoff.remove(&oldest);
            }
        }
        let entry = backoff.entry(address).or_insert(Backoff { failures: 0, last_failure: now, blocked_until: None });
        if entry.is_over(now) {
            entry.failures = 0;
            entry.blocked_until = None;
        }
        entry.failures += 1;
        entry.last_failure = now;
        if entry.failures >= FAILED_HANDSHAKES_BEFORE_BACKOFF {
            entry.blocked_until = Some(now + HANDSHAKE_BACKOFF);
            warn!(service = SERVICE, "Address blocked after failed handshakes");
        }
    }

    /// Counts a new connection as half-open unless too many are (in all, or from this address).
    fn begin_half_open(&self, address: IpAddr) -> bool {
        let mut half_open = lock(&self.half_open);
        let from_address = half_open.get(&address).copied().unwrap_or(0);
        if half_open.values().sum::<usize>() >= MAX_HALF_OPEN_CONNECTIONS || from_address >= MAX_HALF_OPEN_PER_ADDRESS {
            return false;
        }
        half_open.insert(address, from_address + 1);
        true
    }

    fn end_half_open(&self, address: IpAddr) {
        let mut half_open = lock(&self.half_open);
        match half_open.get_mut(&address) {
            Some(count) if *count > 1 => *count -= 1,
            _ => {
                half_open.remove(&address);
            }
        }
    }

    fn handshake_succeeded(&self, address: IpAddr) {
        lock(&self.backoff).remove(&address);
    }
}

fn accept_loop(shared: &Arc<Shared>, listeners: &[TcpListener]) {
    while !shared.stopping.load(Ordering::SeqCst) {
        let mut accepted = false;
        for listener in listeners {
            match listener.accept() {
                Ok((stream, address)) => {
                    accepted = true;
                    admit(shared, stream, address);
                }
                Err(error) if LinkError::is_idle(&error) => {}
                Err(error) => warn!(service = SERVICE, error = %error, "Accept failed"),
            }
        }
        housekeeping(shared);
        if !accepted {
            thread::sleep(POLL / 4);
        }
    }
}

/// Every tick: expire the pairing code, and tell phones when the computer's own listener starts or stops.
fn housekeeping(shared: &Shared) {
    if lock(&shared.pairing).expire(Instant::now()) {
        info!(service = SERVICE, "Pairing code expired");
        shared.pairing_changed();
    }
    let own = shared.deps.gate.own_active_within(OWN_LISTENER_WINDOW);
    let sessions: Vec<(SessionKey, Arc<SessionHandle>)> = lock(&shared.sessions).iter().map(|(key, session)| (*key, session.clone())).collect();
    for (key, session) in sessions {
        if session.own_listener_changed(own) {
            let queued = lock(&shared.queue).len_of(key);
            session.send_load(queued, own);
        }
    }
}

fn admit(shared: &Arc<Shared>, stream: TcpStream, address: SocketAddr) {
    if check_peer(address, shared.allow_loopback).is_err() {
        debug!(service = SERVICE, "Connection refused: not an address on the home network");
        return;
    }
    if !shared.begin_half_open(address.ip()) {
        debug!(service = SERVICE, "Connection refused: too many handshakes under way");
        return;
    }
    let shared_for_error = shared;
    let shared = shared.clone();
    let spawned = thread::Builder::new().name("link-conn".into()).spawn(move || {
        let mut half_open = HalfOpen { shared: &shared, address: address.ip(), done: false };
        if let Err(error) = serve(&shared, stream, address, &mut half_open) {
            debug!(service = SERVICE, error = %error, "Connection ended during set-up");
        }
    });
    if let Err(error) = spawned {
        // The closure (and its `HalfOpen`) never ran.
        shared_for_error.end_half_open(address.ip());
        warn!(service = SERVICE, error = %error, "Cannot start a connection thread");
    }
}

/// Counts the connection as half-open until its handshake is done (or it ends).
struct HalfOpen<'a> {
    shared: &'a Shared,
    address: IpAddr,
    done: bool,
}

impl HalfOpen<'_> {
    fn finish(&mut self) {
        if !std::mem::replace(&mut self.done, true) {
            self.shared.end_half_open(self.address);
        }
    }
}

impl Drop for HalfOpen<'_> {
    fn drop(&mut self) {
        self.finish();
    }
}

fn serve(shared: &Arc<Shared>, stream: TcpStream, address: SocketAddr, half_open: &mut HalfOpen<'_>) -> LinkResult<()> {
    stream.set_nonblocking(false)?;
    prepare(&stream, shared.timing.write_stall)?;
    let deadline = Instant::now() + shared.timing.handshake;
    let prologue = read_prologue(&stream, deadline)?;
    if prologue == PAIR_PROLOGUE {
        serve_pairing(shared, stream, address.ip());
        Ok(())
    } else if prologue == SESSION_PROLOGUE {
        // Only sessions are backed off: pairing has its own limits, and a phone the computer forgot must be able to
        // pair again at once, whatever its failed sessions before.
        if shared.blocked(address.ip()) {
            return Err(LinkError::LimitExceeded("this address is backed off after failed handshakes"));
        }
        session::serve_session(shared, stream, address.ip(), deadline, half_open)
    } else {
        shared.handshake_failed(address.ip());
        // Another major version (or not Lectrice at all): close without answering.
        Err(LinkError::Incompatible("prologue"))
    }
}

/// Reads up to and including the newline, one byte at a time so nothing after it is consumed.
fn read_prologue(mut stream: &TcpStream, deadline: Instant) -> LinkResult<Vec<u8>> {
    let mut line = Vec::with_capacity(MAX_PROLOGUE_BYTES);
    let mut byte = [0u8; 1];
    while line.last() != Some(&b'\n') {
        if line.len() >= MAX_PROLOGUE_BYTES {
            return Err(LinkError::Incompatible("prologue"));
        }
        match stream.read(&mut byte) {
            Ok(0) => return Err(LinkError::Disconnected),
            Ok(_) => line.push(byte[0]),
            Err(error) if LinkError::is_idle(&error) && Instant::now() < deadline => {}
            Err(error) if LinkError::is_idle(&error) => return Err(LinkError::Timeout("prologue")),
            Err(error) => return Err(error.into()),
        }
    }
    Ok(line)
}

fn serve_pairing(shared: &Shared, stream: TcpStream, from: IpAddr) {
    let Some(attempt) = lock(&shared.pairing).begin(from, Instant::now()) else {
        debug!(service = SERVICE, "Pairing attempt refused: no code showing or no free slot");
        let _ = stream.shutdown(Shutdown::Both);
        return;
    };
    shared.pairing_changed();
    let hello = PairHello { device_name: lock(&shared.computer_name).clone(), app_version: shared.app_version.clone(), proto_minor: PROTOCOL_MINOR };
    let mut saved = false;
    // Runs when the phone has proved it knows the code, before it is told so. The window stays locked from the check
    // to `end`: of two attempts with the right code only the first is saved, and none after the window closed (the
    // attempts in the other slots outlive the third wrong code), so the user sees every phone that was paired.
    let mut save = |paired: &Paired| {
        let mut pairing = lock(&shared.pairing);
        if !pairing.is_open_for(&attempt) {
            warn!(service = SERVICE, "Pairing finished after its window closed; the phone isn't saved");
            return Err(LinkError::Timeout("the pairing window closed"));
        }
        shared.deps.peers.put(paired.peer.clone()).inspect_err(|error| warn!(service = SERVICE, error = %error, "Cannot save the paired phone"))?;
        info!(service = SERVICE, peer = paired.peer.peer_id.short(), "Phone paired");
        pairing.end(&attempt, AttemptEnd::Paired(paired.peer.clone()), Instant::now());
        saved = true;
        Ok(())
    };
    let result = stream.try_clone().map_err(LinkError::from).and_then(|mut writer| {
        let mut reader = FrameReader::new(stream);
        let result = pair_as_computer(&mut reader, &mut writer, &attempt.code, &shared.deps.keys, &hello, PAIR_STEP_TIMEOUT, &mut save);
        let _ = writer.shutdown(Shutdown::Both);
        result
    });
    if !saved {
        let end = match result {
            Err(LinkError::PairingFailed(PairingFailure::WrongCode)) => {
                warn!(service = SERVICE, "Wrong pairing code");
                AttemptEnd::WrongCode
            }
            Err(error) => {
                debug!(service = SERVICE, error = %error, "Pairing attempt abandoned");
                AttemptEnd::Abandoned
            }
            Ok(_) => AttemptEnd::Abandoned,
        };
        lock(&shared.pairing).end(&attempt, end, Instant::now());
    }
    shared.pairing_changed();
}
