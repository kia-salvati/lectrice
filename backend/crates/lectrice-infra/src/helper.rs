//! The phone's side of the computer helper (file 24) as the domain's [`SpeechHelper`] port, over `lectrice-link`'s
//! client. One supervisor thread per opened engine connects to the paired computer, opens the phone's exact engine
//! (same id, revision and sample rate, or no clips at all), passes items on and clips back, and reconnects on the
//! 1-2-5-10-30-60 s schedule when the link goes. Every port call returns at once: commands go to that thread.
//!
//! The shell gives the computer's addresses (from NSD or typed), the pinned key, this phone's keys and a socket
//! factory that binds to the Wi-Fi network; it calls [`LinkSpeechHelper::retry_now`] when the network changes, the
//! computer is seen again or play is pressed.

use std::net::SocketAddr;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::sync::lock;
use lectrice_domain::{AppError, AppResult, Audio, ErrorName, HelperEngine, HelperEvent, HelperEvents, HelperItem, HelperState, SpeechHelper};
use lectrice_link::client::{ClientConfig, ClientEvent, ClosedReason, Session, SocketFactory};
use lectrice_link::limits::{MAX_ITEMS_PER_BATCH, MAX_SHORT_TEXT_BYTES, MAX_TEXT_BYTES, reconnect_delay};
use lectrice_link::{ByeReason, CancelScope, Item, LinkError, LinkErrorName, Make, Open, StaticKeys, pcm16_to_f32};
use tracing::{debug, info, warn};

const SERVICE: &str = "LinkSpeechHelper";
const THREAD_NAME: &str = "helper-link";
/// How often the session loop looks for new commands between the computer's events.
const COMMAND_POLL: Duration = Duration::from_millis(20);
/// A session at least this many times `Timing::dead_after` long (30 s by default) proved the link works. A shorter
/// one doesn't restart the reconnect schedule: a computer that takes the phone and drops it at once, in a loop
/// (broken, or hostile after pairing), is retried ever more rarely instead of every second, or without any wait.
const STABLE_SESSION_DEAD_AFTERS: u32 = 2;

/// Where and who the computer is, and how this phone talks to it.
pub struct LinkHelperConfig {
    pub keys: StaticKeys,
    /// The paired computer's pinned static key.
    pub computer_key: [u8; 32],
    /// Tried in order; the shell updates them with [`LinkSpeechHelper::set_addresses`].
    pub addresses: Vec<SocketAddr>,
    pub client: ClientConfig,
    /// Opens the TCP connection (bound to the Wi-Fi network on Android).
    pub sockets: Arc<dyn SocketFactory>,
}

enum Command {
    Make(Vec<HelperItem>),
    CancelAll,
    RetryNow,
    Close,
}

struct Shared {
    config: Mutex<LinkHelperConfig>,
    /// The state, and which supervisor may change it (each `open` and `close` starts a new generation).
    state: Mutex<(HelperState, u64)>,
    commands: Mutex<Option<Sender<Command>>>,
}

impl Shared {
    fn next_generation(&self) -> u64 {
        let mut state = lock(&self.state);
        state.1 += 1;
        state.1
    }
}

pub struct LinkSpeechHelper {
    shared: Arc<Shared>,
}

impl LinkSpeechHelper {
    pub fn new(config: LinkHelperConfig) -> Self {
        Self { shared: Arc::new(Shared { config: Mutex::new(config), state: Mutex::new((HelperState::Off, 0)), commands: Mutex::new(None) }) }
    }

    /// New addresses for the computer (it was found again, or its address changed); tries them at once.
    pub fn set_addresses(&self, addresses: Vec<SocketAddr>) {
        lock(&self.shared.config).addresses = addresses;
        self.retry_now();
    }

    /// Skips the rest of the reconnect wait (Wi-Fi back, the computer seen again, play pressed). Also how a shell
    /// tries again after `NeedsAttention` once the person acted.
    pub fn retry_now(&self) {
        self.send(Command::RetryNow);
    }

    fn send(&self, command: Command) -> bool {
        lock(&self.shared.commands).as_ref().is_some_and(|commands| commands.send(command).is_ok())
    }
}

impl Drop for LinkSpeechHelper {
    fn drop(&mut self) {
        self.send(Command::Close);
    }
}

impl SpeechHelper for LinkSpeechHelper {
    fn state(&self) -> HelperState {
        lock(&self.shared.state).0.clone()
    }

    fn open(&self, engine: HelperEngine, events: Arc<dyn HelperEvents>) -> AppResult<()> {
        let (sender, receiver) = mpsc::channel();
        if let Some(previous) = lock(&self.shared.commands).replace(sender) {
            let _ = previous.send(Command::Close);
        }
        let generation = self.shared.next_generation();
        let supervisor = Supervisor { shared: self.shared.clone(), generation, engine, events, commands: receiver, batch: 0 };
        thread::Builder::new()
            .name(THREAD_NAME.into())
            .spawn(move || supervisor.run())
            .map_err(|error| AppError::new(ErrorName::Internal).with_detail("cannot start the helper link thread").with_source(error))?;
        Ok(())
    }

    fn make(&self, items: Vec<HelperItem>) -> AppResult<()> {
        if !self.state().makes_clips() || !self.send(Command::Make(items)) {
            return Err(AppError::new(ErrorName::HelperUnreachable).with_detail("the computer isn't connected"));
        }
        Ok(())
    }

    fn cancel_all(&self) -> AppResult<()> {
        if !self.send(Command::CancelAll) {
            return Err(AppError::new(ErrorName::HelperUnreachable).with_detail("the helper isn't running"));
        }
        Ok(())
    }

    fn close(&self) {
        if let Some(commands) = lock(&self.shared.commands).take() {
            let _ = commands.send(Command::Close);
        }
        self.shared.next_generation();
        lock(&self.shared.state).0 = HelperState::Off;
    }
}

/// The registry name a link error ends in (C47; `lectrice-link` mirrors the registry's helper names).
pub fn registry_name(name: LinkErrorName) -> ErrorName {
    ErrorName::from_name(name.as_str()).unwrap_or(ErrorName::Internal)
}

/// How a session or a connection attempt ended.
enum End {
    /// `close` was called, or the helper was dropped or opened again.
    Close,
    /// Retry after the schedule's wait.
    Lost,
    /// Retry at once (the computer switched its model).
    Again,
    /// Wait for the shell (`retry_now`): nothing will change by itself.
    Attention(ErrorName),
}

struct Supervisor {
    shared: Arc<Shared>,
    generation: u64,
    engine: HelperEngine,
    events: Arc<dyn HelperEvents>,
    commands: Receiver<Command>,
    batch: u32,
}

impl Supervisor {
    fn run(mut self) {
        let mut failures = 0;
        loop {
            self.set_state(HelperState::Connecting);
            let end = match self.connect() {
                Ok((session, stable_after)) => {
                    let connected = std::time::Instant::now();
                    self.set_state(HelperState::Connected);
                    let end = self.serve(&session);
                    let reason = if matches!(end, End::Close) { ByeReason::Idle } else { ByeReason::Quit };
                    session.close(reason);
                    if connected.elapsed() >= stable_after {
                        failures = 0;
                    }
                    end
                }
                Err(end) => end,
            };
            let waited = match end {
                End::Close => break,
                // At once after a model switch, but only the first time in a row of short sessions.
                End::Again if failures == 0 => {
                    failures = 1;
                    true
                }
                End::Again | End::Lost => {
                    failures += 1;
                    self.set_state(HelperState::Lost);
                    self.wait(Some(reconnect_delay(failures)))
                }
                End::Attention(name) => {
                    self.set_state(HelperState::NeedsAttention(name));
                    self.wait(None)
                }
            };
            if !waited {
                break;
            }
        }
        self.set_state(HelperState::Off);
        debug!(service = SERVICE, "helper link thread ended");
    }

    /// Waits `delay` (or until `retry_now`); `false` once the helper is closed. Items asked for meanwhile are gone:
    /// the app forgot them when the link was lost and asks again after reconnecting.
    fn wait(&self, delay: Option<Duration>) -> bool {
        let deadline = delay.map(|delay| std::time::Instant::now() + delay);
        loop {
            let command = match deadline {
                Some(deadline) => match self.commands.recv_timeout(deadline.saturating_duration_since(std::time::Instant::now())) {
                    Ok(command) => command,
                    Err(RecvTimeoutError::Timeout) => return true,
                    Err(RecvTimeoutError::Disconnected) => return false,
                },
                None => match self.commands.recv() {
                    Ok(command) => command,
                    Err(_) => return false,
                },
            };
            match command {
                Command::Close => return false,
                Command::RetryNow => return true,
                Command::Make(_) | Command::CancelAll => {}
            }
        }
    }

    /// Tries every address; the first computer that opens exactly this engine wins. Also says how long the session
    /// must last to count as stable.
    fn connect(&self) -> Result<(Session, Duration), End> {
        let (addresses, keys, computer_key, client, sockets) = {
            let config = lock(&self.shared.config);
            (config.addresses.clone(), config.keys.clone(), config.computer_key, config.client.clone(), config.sockets.clone())
        };
        let mut end = End::Lost;
        for address in addresses {
            let opened = Session::connect_via(sockets.as_ref(), address, &keys, &computer_key, &client).and_then(|session| self.open(session, &client));
            match opened {
                Ok(session) => return Ok((session, client.timing.dead_after * STABLE_SESSION_DEAD_AFTERS)),
                Err(error) => {
                    let name = error.name();
                    info!(service = SERVICE, error = %error, name = name.as_str(), "helper connection failed");
                    if name != LinkErrorName::HelperUnreachable {
                        end = End::Attention(registry_name(name));
                    }
                }
            }
        }
        Err(end)
    }

    /// Asks for the phone's engine and checks the answer: a computer that says another engine, revision or sample
    /// rate never makes a clip for this phone (file 24 §3.11).
    fn open(&self, session: Session, client: &ClientConfig) -> Result<Session, LinkError> {
        let open = Open { engine_id: self.engine.engine_id.clone(), revision: self.engine.revision.clone(), voice_ids: Vec::new() };
        let ready = session.open(open, client.timing.handshake)?;
        if ready.engine_id != self.engine.engine_id || ready.revision != self.engine.revision || ready.sample_rate != self.engine.sample_rate {
            warn!(service = SERVICE, engine_id = %ready.engine_id, revision = %ready.revision, sample_rate = ready.sample_rate, "computer opened another engine");
            session.close(ByeReason::Idle);
            return Err(LinkError::Incompatible("the computer opened another engine"));
        }
        info!(service = SERVICE, peer = session.computer().short(), engine_id = %ready.engine_id, "helper connected");
        Ok(session)
    }

    fn serve(&mut self, session: &Session) -> End {
        loop {
            loop {
                match self.commands.try_recv() {
                    Ok(Command::Make(items)) => {
                        if self.make(session, items).is_err() {
                            return End::Lost;
                        }
                    }
                    Ok(Command::CancelAll) => {
                        if session.cancel(CancelScope::All).is_err() {
                            return End::Lost;
                        }
                    }
                    Ok(Command::RetryNow) => {}
                    Ok(Command::Close) | Err(TryRecvError::Disconnected) => return End::Close,
                    Err(TryRecvError::Empty) => break,
                }
            }
            let Some(event) = session.next_event(COMMAND_POLL) else { continue };
            match event {
                ClientEvent::Clip { item_id, sample_rate, samples, make_ms } => {
                    debug!(service = SERVICE, item_id, make_ms, samples = samples.len(), "clip received");
                    let audio = Audio { samples: samples.into_iter().map(pcm16_to_f32).collect(), sample_rate };
                    self.events.on_event(HelperEvent::Clip { item_id, audio });
                }
                ClientEvent::ItemFailed { item_id, error } => {
                    let name = ErrorName::from_name(&error.name).unwrap_or(ErrorName::Internal);
                    self.events.on_event(HelperEvent::ItemFailed { item_id, error: name });
                }
                ClientEvent::Load(load) => self.set_state(if load.own_listener { HelperState::Busy } else { HelperState::Connected }),
                ClientEvent::Closed(reason) => {
                    info!(service = SERVICE, reason = ?reason, "helper session ended");
                    return match reason {
                        ClosedReason::Bye(ByeReason::Unpaired) => End::Attention(ErrorName::HelperNotPaired),
                        ClosedReason::Bye(ByeReason::EngineSwitch) => End::Again,
                        ClosedReason::Local => End::Close,
                        _ => End::Lost,
                    };
                }
            }
        }
    }

    /// Sends items in batches the protocol accepts. An item the protocol would refuse (it would end the session for
    /// everyone) fails here, so the app makes it itself.
    fn make(&mut self, session: &Session, items: Vec<HelperItem>) -> Result<(), LinkError> {
        let (fitting, too_large): (Vec<HelperItem>, Vec<HelperItem>) =
            items.into_iter().partition(|item| item.text.len() <= MAX_TEXT_BYTES && item.voice_id.as_str().len() <= MAX_SHORT_TEXT_BYTES);
        for item in too_large {
            self.events.on_event(HelperEvent::ItemFailed { item_id: item.item_id, error: ErrorName::RequestInvalid });
        }
        for chunk in fitting.chunks(MAX_ITEMS_PER_BATCH) {
            self.batch = self.batch.wrapping_add(1);
            let items = chunk
                .iter()
                .map(|item| Item { item_id: item.item_id, voice_id: item.voice_id.as_str().to_string(), text: item.text.clone(), urgent: item.urgent })
                .collect();
            session.make(Make { batch_id: self.batch, items })?;
        }
        Ok(())
    }

    /// A closed supervisor, or one replaced by a newer `open`, no longer changes the state or reports it.
    fn set_state(&self, next: HelperState) {
        {
            let mut state = lock(&self.shared.state);
            if state.1 != self.generation || state.0 == next {
                return;
            }
            info!(service = SERVICE, from = ?state.0, to = ?next, "helper link state changed");
            state.0 = next.clone();
        }
        self.events.on_event(HelperEvent::State(next));
    }
}
