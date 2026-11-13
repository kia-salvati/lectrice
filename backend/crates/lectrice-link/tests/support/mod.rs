//! A computer with a fake engine and a phone client, both in this process over loopback.

#![allow(dead_code)]

use std::collections::HashMap;
use std::net::{SocketAddr, TcpListener};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use lectrice_link::client::{ClientConfig, ClientEvent, Session, pair};
use lectrice_link::limits::Timing;
use lectrice_link::server::{ClipEngine, CpuGate, EngineProvider, Server, ServerConfig, ServerDeps, ServerEvent, ServerObserver};
use lectrice_link::{EngineOffer, ItemError, LinkResult, Open, PairingCode, PeerId, PeerRecord, PeerStore, Refusal, StaticKeys};

pub const ENGINE_ID: &str = "kokoro-v1.0-lectrice-fast-v1";
pub const REVISION: &str = "sherpa-onnx-1.13.8/kokoro-1";
pub const SAMPLE_RATE: u32 = 24_000;
pub const VOICE: &str = "af_heart";

pub fn timing() -> Timing {
    Timing {
        handshake: Duration::from_secs(3),
        ping_every: Duration::from_millis(200),
        dead_after: Duration::from_secs(1),
        write_stall: Duration::from_secs(2),
    }
}

/// Deterministic audio: the same text always gives the same samples (so a clip made twice is byte-identical).
pub fn fake_samples(text: &str) -> Vec<f32> {
    let mut seed = text.bytes().fold(0x811c_9dc5_u32, |hash, byte| (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193));
    let length = 2_400 + text.len() * 400;
    (0..length)
        .map(|index| {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            let tone = (index as f32 * 0.05).sin() * 0.3;
            tone + ((seed % 1000) as f32 / 1000.0 - 0.5) * 0.02
        })
        .collect()
}

pub struct FakeEngine {
    pub delay: Duration,
    pub made: AtomicUsize,
    /// The order texts were made in.
    pub log: Mutex<Vec<String>>,
}

impl FakeEngine {
    pub fn new(delay: Duration) -> Arc<Self> {
        Arc::new(Self { delay, made: AtomicUsize::new(0), log: Mutex::new(Vec::new()) })
    }
}

impl ClipEngine for FakeEngine {
    fn engine_id(&self) -> String {
        ENGINE_ID.into()
    }
    fn revision(&self) -> String {
        REVISION.into()
    }
    fn sample_rate(&self) -> u32 {
        SAMPLE_RATE
    }
    fn voices(&self) -> Vec<String> {
        vec![VOICE.into(), "bf_emma".into()]
    }
    fn synthesize(&self, text: &str, _voice_id: &str) -> Result<Vec<f32>, ItemError> {
        std::thread::sleep(self.delay);
        self.made.fetch_add(1, Ordering::SeqCst);
        self.log.lock().unwrap().push(text.to_owned());
        if text.contains("FAIL") {
            return Err(ItemError { name: "SpeechSynthesisFailed".into(), code: 2003 });
        }
        Ok(fake_samples(text))
    }
}

pub struct FakeProvider {
    pub engine: Arc<FakeEngine>,
}

impl EngineProvider for FakeProvider {
    fn offers(&self) -> Vec<EngineOffer> {
        vec![EngineOffer { engine_id: ENGINE_ID.into(), revision: REVISION.into(), installed: true, sample_rate: SAMPLE_RATE }]
    }
    fn open(&self, engine_id: &str, revision: &str) -> Result<Arc<dyn ClipEngine>, Refusal> {
        if engine_id != ENGINE_ID {
            return Err(Refusal::ModelMissing { bundle_id: "kokoro-en-v1_0-fp32".into() });
        }
        if revision != REVISION {
            return Err(Refusal::RevisionMismatch { revision: REVISION.into() });
        }
        Ok(self.engine.clone())
    }
}

#[derive(Default)]
pub struct MemoryPeers {
    pub peers: Mutex<HashMap<PeerId, PeerRecord>>,
}

impl PeerStore for MemoryPeers {
    fn find(&self, key: &[u8; 32]) -> LinkResult<Option<PeerRecord>> {
        Ok(self.peers.lock().unwrap().values().find(|peer| peer.static_public == *key).cloned())
    }
    fn put(&self, peer: PeerRecord) -> LinkResult<()> {
        self.peers.lock().unwrap().insert(peer.peer_id.clone(), peer);
        Ok(())
    }
    fn remove(&self, peer_id: &PeerId) -> LinkResult<bool> {
        Ok(self.peers.lock().unwrap().remove(peer_id).is_some())
    }
}

#[derive(Default)]
pub struct Events {
    pub events: Mutex<Vec<ServerEvent>>,
}

impl ServerObserver for Events {
    fn changed(&self, event: ServerEvent) {
        self.events.lock().unwrap().push(event);
    }
}

impl Events {
    pub fn any(&self, matches: impl Fn(&ServerEvent) -> bool) -> bool {
        self.events.lock().unwrap().iter().any(matches)
    }
}

pub struct Computer {
    pub server: Option<Server>,
    pub address: SocketAddr,
    pub keys: StaticKeys,
    pub peers: Arc<MemoryPeers>,
    pub engine: Arc<FakeEngine>,
    pub gate: Arc<CpuGate>,
    pub events: Arc<Events>,
}

impl Computer {
    pub fn start(delay: Duration) -> Self {
        let keys = StaticKeys::generate().unwrap();
        let peers = Arc::new(MemoryPeers::default());
        let engine = FakeEngine::new(delay);
        let gate = Arc::new(CpuGate::new());
        let events = Arc::new(Events::default());
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let deps = ServerDeps {
            keys: keys.clone(),
            peers: peers.clone(),
            engines: Arc::new(FakeProvider { engine: engine.clone() }),
            gate: gate.clone(),
            observer: events.clone(),
        };
        let config = ServerConfig { computer_name: "Test laptop".into(), app_version: "0.2.0".into(), timing: timing(), allow_loopback: true };
        let server = Server::start(vec![listener], deps, config).unwrap();
        let address = server.local_addresses()[0];
        Self { server: Some(server), address, keys, peers, engine, gate, events }
    }

    pub fn server(&self) -> &Server {
        self.server.as_ref().unwrap()
    }

    /// The code the computer shows after "Pair a phone".
    pub fn open_pairing(&self) -> PairingCode {
        match self.server().start_pairing().unwrap() {
            lectrice_link::server::PairingStatus::Showing { code, .. } => code,
            other => panic!("pairing didn't open: {other:?}"),
        }
    }
}

pub struct Phone {
    pub keys: StaticKeys,
    pub config: ClientConfig,
    pub computer_key: Option<[u8; 32]>,
}

impl Phone {
    pub fn new(name: &str) -> Self {
        Self {
            keys: StaticKeys::generate().unwrap(),
            config: ClientConfig { device_name: name.into(), app_version: "0.4.0".into(), timing: timing(), allow_loopback: true },
            computer_key: None,
        }
    }

    pub fn pair_with(&mut self, computer: &Computer, address: SocketAddr) {
        let code = computer.open_pairing();
        let paired = pair(address, &code, &self.keys, &self.config).unwrap();
        self.computer_key = Some(paired.peer.static_public);
    }

    pub fn connect(&self, address: SocketAddr) -> LinkResult<Session> {
        Session::connect(address, &self.keys, self.computer_key.as_ref().unwrap(), &self.config)
    }

    pub fn open_session(&self, address: SocketAddr) -> Session {
        let session = self.connect(address).unwrap();
        session.open(open_request(), Duration::from_secs(3)).unwrap();
        session
    }
}

pub fn open_request() -> Open {
    Open { engine_id: ENGINE_ID.into(), revision: REVISION.into(), voice_ids: vec![VOICE.into()] }
}

pub fn item(item_id: u32, urgent: bool) -> lectrice_link::Item {
    lectrice_link::Item { item_id, voice_id: VOICE.into(), text: sentence(item_id), urgent }
}

/// Invented text, different for every id.
pub fn sentence(item_id: u32) -> String {
    format!("Sentence number {item_id} is about a small garden path.")
}

/// Waits for the next clip or failure (skipping `Load`), up to `timeout`.
pub fn next_result(session: &Session, timeout: Duration) -> Option<ClientEvent> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        match session.next_event(deadline.saturating_duration_since(Instant::now())) {
            Some(ClientEvent::Load(_)) => {}
            other => return other,
        }
    }
    None
}

pub fn wait_until(timeout: Duration, mut condition: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if condition() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    condition()
}
