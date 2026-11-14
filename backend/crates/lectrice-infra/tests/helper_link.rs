//! The computer helper end to end over real sockets (file 24 §13): a `lectrice-link` server with a fake engine on one
//! side; on the phone side the real `LinkSpeechHelper`, `HelperFill`, `SpeakSentence`, the file audio cache and the WAV
//! encoder. `FaultyLink` sits in between to cut the link. Invented text only.

use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use lectrice_app::speech::{
    HelperFill, HelperFillDeps, HelperWait, ListenerPosition, ReadAhead, ReadAheadDeps, ReadAheadSettings, SpeakSentence, SpeakSentenceArgs, SpeakSentenceDeps,
    SpeechWorker,
};
use lectrice_app::{RequestContext, UseCase};
use lectrice_domain::{
    AppResult, Audio, AudioCache, AudioDecoder, AudioKey, AudioRetention, Block, BlockKind, Document, DocumentBytes, DocumentId, DocumentReader,
    DocumentSummary, ErrorName, HelperState, ListOptions, Page, Sentence, SentenceId, SpeechEngine, SpeechRequest, Timestamp, Voice, VoiceGender, VoiceId,
    VoiceLanguage,
};
use lectrice_infra::helper::{LinkHelperConfig, LinkSpeechHelper};
use lectrice_infra::{FsAudioCache, WavDecoder, WavEncoder};
use lectrice_link::client::{ClientConfig, SocketFactory, pair};
use lectrice_link::limits::Timing;
use lectrice_link::server::{ClipEngine, CpuGate, EngineProvider, PairingStatus, Server, ServerConfig, ServerDeps, ServerEvent, ServerObserver};
use lectrice_link::testing::FaultyLink;
use lectrice_link::{EngineOffer, ItemError, LinkResult, PeerId, PeerRecord, PeerStore, Refusal, StaticKeys, pcm16_from_f32};
use tempfile::TempDir;

const ENGINE_ID: &str = "kokoro-v1.0-lectrice-fast-v1";
const REVISION: &str = "sherpa-onnx-1.13.8/kokoro-1";
const SAMPLE_RATE: u32 = 24_000;
const VOICE: &str = "af_heart";
const WAIT: Duration = Duration::from_secs(10);

fn timing() -> Timing {
    Timing {
        handshake: Duration::from_secs(3),
        ping_every: Duration::from_millis(200),
        dead_after: Duration::from_secs(1),
        write_stall: Duration::from_secs(2),
    }
}

/// The same text always gives the same samples.
fn fake_samples(text: &str) -> Vec<f32> {
    let mut seed = text.bytes().fold(0x811c_9dc5_u32, |hash, byte| (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193));
    (0..2_400 + text.len() * 200)
        .map(|index| {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            (index as f32 * 0.05).sin() * 0.3 + ((seed % 1000) as f32 / 1000.0 - 0.5) * 0.02
        })
        .collect()
}

fn sentence_text(id: u32) -> String {
    format!("Sentence {id} walks along the quiet harbour path.")
}

// ---- the computer ----

struct DesktopEngine {
    revision: &'static str,
    made: AtomicUsize,
}

impl ClipEngine for DesktopEngine {
    fn engine_id(&self) -> String {
        ENGINE_ID.into()
    }
    fn revision(&self) -> String {
        self.revision.into()
    }
    fn sample_rate(&self) -> u32 {
        SAMPLE_RATE
    }
    fn voices(&self) -> Vec<String> {
        vec![VOICE.into()]
    }
    fn synthesize(&self, text: &str, _voice_id: &str) -> Result<Vec<f32>, ItemError> {
        std::thread::sleep(Duration::from_millis(5));
        self.made.fetch_add(1, Ordering::SeqCst);
        Ok(fake_samples(text))
    }
}

/// `has_model: false` refuses like a computer without the phone's model; the engine's revision may differ from what
/// was asked (a broken or older computer that doesn't check).
struct Provider {
    engine: Arc<DesktopEngine>,
    has_model: bool,
}

impl EngineProvider for Provider {
    fn offers(&self) -> Vec<EngineOffer> {
        vec![EngineOffer { engine_id: ENGINE_ID.into(), revision: self.engine.revision.into(), installed: self.has_model, sample_rate: SAMPLE_RATE }]
    }
    fn open(&self, _engine_id: &str, _revision: &str) -> Result<Arc<dyn ClipEngine>, Refusal> {
        if !self.has_model {
            return Err(Refusal::ModelMissing { bundle_id: "kokoro-en-v1_0-fast".into() });
        }
        Ok(self.engine.clone())
    }
}

#[derive(Default)]
struct Peers(Mutex<Vec<PeerRecord>>);

impl PeerStore for Peers {
    fn find(&self, key: &[u8; 32]) -> LinkResult<Option<PeerRecord>> {
        Ok(self.0.lock().unwrap().iter().find(|peer| peer.static_public == *key).cloned())
    }
    fn put(&self, peer: PeerRecord) -> LinkResult<()> {
        self.0.lock().unwrap().push(peer);
        Ok(())
    }
    fn remove(&self, peer_id: &PeerId) -> LinkResult<bool> {
        let mut peers = self.0.lock().unwrap();
        let before = peers.len();
        peers.retain(|peer| peer.peer_id != *peer_id);
        Ok(peers.len() < before)
    }
}

struct Quiet;

impl ServerObserver for Quiet {
    fn changed(&self, _event: ServerEvent) {}
}

struct Computer {
    server: Server,
    address: SocketAddr,
    engine: Arc<DesktopEngine>,
}

impl Computer {
    fn start(revision: &'static str, has_model: bool) -> Self {
        let engine = Arc::new(DesktopEngine { revision, made: AtomicUsize::new(0) });
        let deps = ServerDeps {
            keys: StaticKeys::generate().unwrap(),
            peers: Arc::new(Peers::default()),
            engines: Arc::new(Provider { engine: engine.clone(), has_model }),
            gate: Arc::new(CpuGate::new()),
            observer: Arc::new(Quiet),
        };
        let config = ServerConfig { computer_name: "Test laptop".into(), app_version: "0.4.0".into(), timing: timing(), allow_loopback: true };
        let server = Server::start(vec![TcpListener::bind("127.0.0.1:0").unwrap()], deps, config).unwrap();
        let address = server.local_addresses()[0];
        Self { server, address, engine }
    }
}

// ---- the phone ----

struct PhoneEngine {
    synthesized: AtomicUsize,
}

impl SpeechEngine for PhoneEngine {
    fn id(&self) -> &str {
        ENGINE_ID
    }
    fn voices(&self) -> Vec<Voice> {
        vec![Voice { id: VoiceId::new(VOICE), name: "Heart".into(), language: VoiceLanguage::EnUs, gender: VoiceGender::Female }]
    }
    fn sample_rate(&self) -> u32 {
        SAMPLE_RATE
    }
    fn revision(&self) -> &str {
        REVISION
    }
    fn synthesize(&self, request: SpeechRequest) -> AppResult<Audio> {
        self.synthesized.fetch_add(1, Ordering::SeqCst);
        Ok(Audio { samples: vec![0.0; request.text.len() * 10], sample_rate: SAMPLE_RATE })
    }
}

struct OneBook(Document);

impl DocumentReader for OneBook {
    fn find(&self, id: &DocumentId) -> AppResult<Option<Document>> {
        Ok((*id == self.0.id).then(|| self.0.clone()))
    }
    fn find_summary(&self, id: &DocumentId) -> AppResult<Option<DocumentSummary>> {
        let at = Timestamp::UNIX_EPOCH;
        Ok((*id == self.0.id).then(|| DocumentSummary {
            id: id.clone(),
            title: "T".into(),
            source_file_name: "t.pdf".into(),
            page_count: 1,
            sentence_count: 0,
            cleaner_version: 1,
            added_at: at,
            last_opened_at: at,
            last_sentence_id: None,
        }))
    }
    fn find_original_pdf(&self, _id: &DocumentId) -> AppResult<Option<Vec<u8>>> {
        Ok(None)
    }
    fn list_recent(&self, options: ListOptions) -> AppResult<Page<DocumentSummary>> {
        Ok(Page { items: Vec::new(), total: 0, limit: options.limit_or_default(), offset: 0 })
    }
    fn list_removed(&self) -> AppResult<Vec<DocumentId>> {
        Ok(Vec::new())
    }
    fn library_usage(&self) -> AppResult<Vec<DocumentBytes>> {
        Ok(Vec::new())
    }
}

fn book() -> Document {
    let blocks = (0..300)
        .map(|id| Block {
            id,
            kind: BlockKind::Paragraph,
            level: None,
            page_index: 1,
            spoken: true,
            text: None,
            sentences: vec![Sentence { id: SentenceId(id), text: sentence_text(id), page_index: 1, speech_text: None }],
            cue: None,
        })
        .collect();
    Document {
        id: DocumentId::parse(&format!("{:064x}", 5)).unwrap(),
        title: "T".into(),
        source_file_name: "t.pdf".into(),
        page_count: 1,
        cleaner_version: 1,
        pages: Vec::new(),
        toc: Vec::new(),
        blocks,
    }
}

#[derive(Default)]
struct CountingSockets(AtomicUsize);

impl SocketFactory for CountingSockets {
    fn connect(&self, address: SocketAddr, timeout: Duration) -> std::io::Result<TcpStream> {
        self.0.fetch_add(1, Ordering::SeqCst);
        TcpStream::connect_timeout(&address, timeout)
    }
}

struct Phone {
    _dir: TempDir,
    document: Document,
    engine: Arc<PhoneEngine>,
    cache: Arc<FsAudioCache>,
    sockets: Arc<CountingSockets>,
    helper: Arc<LinkSpeechHelper>,
    read_ahead: Arc<ReadAhead>,
    fill: Arc<HelperFill>,
    speak: SpeakSentence,
}

impl Phone {
    /// Pairs with `computer`, then talks to it through `address` (the computer itself or a `FaultyLink`).
    fn pair(computer: &Computer, address: SocketAddr) -> Self {
        let keys = StaticKeys::generate().unwrap();
        let client = ClientConfig { device_name: "Test phone".into(), app_version: "0.4.0".into(), timing: timing(), allow_loopback: true };
        let code = match computer.server.start_pairing().unwrap() {
            PairingStatus::Showing { code, .. } => code,
            other => panic!("pairing didn't open: {other:?}"),
        };
        let paired = pair(computer.address, &code, &keys, &client).unwrap();
        let sockets = Arc::new(CountingSockets::default());
        let helper = Arc::new(LinkSpeechHelper::new(LinkHelperConfig {
            keys,
            computer_key: paired.peer.static_public,
            addresses: vec![address],
            client,
            sockets: sockets.clone(),
        }));

        let dir = TempDir::new().unwrap();
        let document = book();
        let reader: Arc<dyn DocumentReader> = Arc::new(OneBook(document.clone()));
        let engine = Arc::new(PhoneEngine { synthesized: AtomicUsize::new(0) });
        let cache = Arc::new(FsAudioCache::open(dir.path(), u64::MAX).unwrap());
        let encoder = Arc::new(WavEncoder);
        let worker = Arc::new(SpeechWorker::start(engine.clone()).unwrap());
        let settings = ReadAheadSettings { read_ahead_minutes: 1, keep_minutes_per_document: 60 };
        let read_ahead_deps =
            ReadAheadDeps { worker: worker.clone(), reader: reader.clone(), cache: cache.clone(), retention: cache.clone(), encoder: encoder.clone() };
        let read_ahead = Arc::new(ReadAhead::new(read_ahead_deps, settings).unwrap());
        let fill = HelperFill::new(HelperFillDeps {
            helper: helper.clone(),
            engine: engine.clone(),
            reader: reader.clone(),
            cache: cache.clone(),
            retention: cache.clone(),
            encoder: encoder.clone(),
            read_ahead: read_ahead.clone(),
        });
        let speak = SpeakSentence::new(SpeakSentenceDeps {
            engine: engine.clone(),
            worker,
            reader,
            cache: cache.clone(),
            encoder,
            read_ahead: read_ahead.clone(),
            helper: Some(fill.clone()),
            helper_wait: HelperWait { grace: Duration::from_secs(3), cold_grace: Duration::from_secs(3) },
        });
        Self { _dir: dir, document, engine, cache, sockets, helper, read_ahead, fill, speak }
    }

    fn key(&self, sentence: u32) -> AudioKey {
        AudioKey {
            document_id: self.document.id.clone(),
            cleaner_version: 1,
            engine_id: ENGINE_ID.into(),
            voice_id: VoiceId::new(VOICE),
            sentence_id: SentenceId(sentence),
        }
    }

    fn follow(&self, sentence: u32) {
        let key = self.key(sentence);
        self.fill.follow(ListenerPosition { document: &self.document, key: &key, rate: 1.0, read_skipped: false });
    }

    fn cached(&self, sentence: u32) -> bool {
        self.cache.contains(&self.key(sentence)).unwrap()
    }

    fn state(&self) -> HelperState {
        self.fill.status().state
    }

    /// The cached clip holds exactly the samples the computer made (FLAC is lossless, the WAV is the same PCM).
    fn assert_exact(&self, sentence: u32) {
        let bytes = self.cache.get(&self.key(sentence)).unwrap().expect("cached");
        let decoded: Vec<i16> = WavDecoder.decode(&bytes).unwrap().samples.into_iter().map(pcm16_from_f32).collect();
        let made: Vec<i16> = fake_samples(&sentence_text(sentence)).into_iter().map(pcm16_from_f32).collect();
        assert_eq!(decoded, made, "sentence {sentence}");
    }
}

fn wait_until(what: &str, mut condition: impl FnMut() -> bool) {
    let started = Instant::now();
    while !condition() {
        assert!(started.elapsed() < WAIT, "timed out waiting for: {what}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn the_computer_fills_the_phones_cache_with_exact_clips_and_the_phones_engine_stays_idle() {
    let computer = Computer::start(REVISION, true);
    let phone = Phone::pair(&computer, computer.address);

    phone.fill.start().unwrap();
    wait_until("connected", || phone.state() == HelperState::Connected);
    phone.follow(0);
    wait_until("40 sentences cached", || (1..=40).all(|sentence| phone.cached(sentence)));

    for sentence in [1, 17, 40] {
        phone.assert_exact(sentence);
    }
    assert!(phone.read_ahead.is_paused(&phone.document.id));
    assert_eq!(phone.engine.synthesized.load(Ordering::SeqCst), 0, "C46: the phone's model is never needed");
    assert!(phone.sockets.0.load(Ordering::SeqCst) >= 1, "the shell's socket factory opened the connection");
}

/// C45 end to end: a miss is sent urgent and the computer's clip is played within the grace.
#[test]
fn a_sentence_the_player_needs_now_comes_from_the_computer() {
    let computer = Computer::start(REVISION, true);
    let phone = Phone::pair(&computer, computer.address);
    phone.fill.start().unwrap();
    wait_until("connected", || phone.state() == HelperState::Connected);

    let args = SpeakSentenceArgs {
        document_id: phone.document.id.clone(),
        sentence_id: SentenceId(200),
        voice_id: VoiceId::new(VOICE),
        rate: 1.0,
        read_skipped: false,
        plan_ahead: true,
    };
    let spoken = phone.speak.run(&RequestContext::new("test"), args).unwrap();

    assert_eq!(spoken.mime_type, "audio/wav");
    phone.assert_exact(200);
    assert_eq!(phone.engine.synthesized.load(Ordering::SeqCst), 0);
}

/// Cut, noticed, the phone takes the book back; healed, the helper reconnects and fills on, with no broken clip.
#[test]
fn a_cut_link_hands_the_book_to_the_phone_and_the_helper_comes_back() {
    let computer = Computer::start(REVISION, true);
    let link = FaultyLink::start(computer.address).unwrap();
    let phone = Phone::pair(&computer, link.address());
    phone.fill.start().unwrap();
    wait_until("connected", || phone.state() == HelperState::Connected);
    phone.follow(0);
    wait_until("a few sentences cached", || (1..=5).all(|sentence| phone.cached(sentence)));

    link.cut();
    wait_until("lost", || phone.state() == HelperState::Lost);
    assert!(!phone.read_ahead.is_paused(&phone.document.id), "the local read-ahead has the book");

    link.heal();
    phone.helper.retry_now();
    wait_until("connected again", || phone.state() == HelperState::Connected);
    phone.follow(100);
    wait_until("filling from the new place", || (101..=120).all(|sentence| phone.cached(sentence)));

    assert!(phone.read_ahead.is_paused(&phone.document.id));
    for sentence in (1..=5).chain(101..=120) {
        phone.assert_exact(sentence);
    }
}

/// File 24 §3.11: a computer that answers with another revision never puts a clip in the phone's cache.
#[test]
fn a_computer_with_another_revision_or_without_the_model_needs_attention_and_fills_nothing() {
    for (revision, has_model, expected) in
        [("sherpa-onnx-1.13.9/kokoro-1", true, ErrorName::HelperIncompatible), (REVISION, false, ErrorName::HelperModelMissing)]
    {
        let computer = Computer::start(revision, has_model);
        let phone = Phone::pair(&computer, computer.address);

        phone.fill.start().unwrap();
        wait_until("needs attention", || phone.state() == HelperState::NeedsAttention(expected));
        phone.follow(0);
        std::thread::sleep(Duration::from_millis(100));

        assert!(!phone.cached(1));
        assert_eq!(computer.engine.made.load(Ordering::SeqCst), 0);
        assert!(!phone.read_ahead.is_paused(&phone.document.id));
    }
}

#[test]
fn a_phone_the_computer_forgot_needs_attention() {
    let computer = Computer::start(REVISION, true);
    let phone = Phone::pair(&computer, computer.address);
    phone.fill.start().unwrap();
    wait_until("connected", || phone.state() == HelperState::Connected);

    let peer = computer.server.sessions()[0].peer_id.clone();
    computer.server.forget(&peer).unwrap();

    wait_until("not paired", || phone.state() == HelperState::NeedsAttention(ErrorName::HelperNotPaired));
}

/// A paired computer that takes the phone and drops it at once, every time (broken, or hostile): the phone must
/// come back ever more rarely, not in a loop that keeps the radio and a core busy.
#[test]
fn a_computer_that_drops_every_session_at_once_is_retried_on_the_schedule() {
    use lectrice_domain::{HelperEngine, HelperEvent, HelperEvents, SpeechHelper};
    use lectrice_link::client::DirectSockets;
    use lectrice_link::testing::RawComputer;
    use lectrice_link::{ByeReason, Ready, ServerMessage, Welcome};

    struct Ignored;
    impl HelperEvents for Ignored {
        fn on_event(&self, _event: HelperEvent) {}
    }

    let computer_keys = StaticKeys::generate().unwrap();
    let computer = RawComputer::bind(computer_keys.clone()).unwrap();
    let address = computer.address().unwrap();
    let sessions = Arc::new(AtomicUsize::new(0));
    let counted = sessions.clone();
    std::thread::spawn(move || {
        loop {
            let welcome = Welcome { proto_minor: 0, app_version: "0.2.0".into(), computer_name: "Laptop".into(), engines: Vec::new() };
            let Ok(mut session) = computer.accept(welcome) else { return };
            counted.fetch_add(1, Ordering::SeqCst);
            let _ = session.receive(WAIT);
            let ready = Ready { engine_id: ENGINE_ID.into(), revision: REVISION.into(), sample_rate: SAMPLE_RATE, voices: vec![VOICE.into()] };
            let _ = session.send(&ServerMessage::Ready(ready));
            let _ = session.send(&ServerMessage::Bye(ByeReason::EngineSwitch));
        }
    });
    let started = Instant::now();
    let helper = LinkSpeechHelper::with_reconnect_schedule(
        LinkHelperConfig {
            keys: StaticKeys::generate().unwrap(),
            computer_key: *computer_keys.public(),
            addresses: vec![address],
            client: ClientConfig { device_name: "Phone".into(), app_version: "0.4.0".into(), timing: timing(), allow_loopback: true },
            sockets: Arc::new(DirectSockets),
        },
        short_schedule,
    );
    helper.open(HelperEngine { engine_id: ENGINE_ID.into(), revision: REVISION.into(), sample_rate: SAMPLE_RATE }, Arc::new(Ignored)).unwrap();

    // The first drop is answered at once (a model switch looks like this), then each wait follows the schedule.
    wait_until("four sessions", || sessions.load(Ordering::SeqCst) >= 4);
    let scheduled = short_schedule(2) + short_schedule(3);
    assert!(started.elapsed() >= scheduled, "reconnected after {:?}, the schedule waits {scheduled:?}", started.elapsed());
    helper.close();
}

/// The reconnect schedule in tenths of a second instead of seconds.
fn short_schedule(failures: u32) -> Duration {
    lectrice_link::limits::reconnect_delay(failures) / 10
}
