//! `FaultyLink` (feature `testing`, file 24 §13): a TCP proxy on loopback between a phone client and a computer
//! server that can cut the link (both ways or one way), stall it (bytes held, no reset), throttle it, add latency with
//! jitter, and cut after a number of bytes towards the phone (mid-chunk). For tests only.
//! [`RawPhone`] and [`RawComputer`] are hand-driven peers for the malicious-peer tests.

mod raw;

pub use raw::{RawComputer, RawComputerSession, RawPhone};

use std::io::{self, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::Duration;

const PUMP_POLL: Duration = Duration::from_millis(10);
const PUMP_CHUNK: usize = 4096;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Direction {
    #[default]
    Both,
    ToPhone,
    ToComputer,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Faults {
    /// Live connections are closed and new ones refused, in this direction.
    pub cut: Option<Direction>,
    /// Bytes stop moving (held in the kernel buffers), nothing is closed.
    pub stall: bool,
    /// Bytes per second per direction.
    pub throttle: Option<u64>,
    pub latency: Duration,
    /// Each chunk waits `latency ± jitter`.
    pub jitter: Duration,
    /// After this many more bytes towards the phone, the connection is cut.
    pub cut_after_bytes_to_phone: Option<u64>,
}

struct State {
    faults: Mutex<Faults>,
    to_phone_bytes: AtomicU64,
    stopping: AtomicBool,
    connections: Mutex<Vec<TcpStream>>,
    jitter_seed: AtomicU64,
}

pub struct FaultyLink {
    address: SocketAddr,
    state: Arc<State>,
}

impl FaultyLink {
    /// Listens on `127.0.0.1:0` and forwards every connection to `target`.
    pub fn start(target: SocketAddr) -> io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let address = listener.local_addr()?;
        listener.set_nonblocking(true)?;
        let state = Arc::new(State {
            faults: Mutex::new(Faults::default()),
            to_phone_bytes: AtomicU64::new(0),
            stopping: AtomicBool::new(false),
            connections: Mutex::new(Vec::new()),
            jitter_seed: AtomicU64::new(0x9e37_79b9_7f4a_7c15),
        });
        let accept_state = state.clone();
        thread::spawn(move || accept(&listener, target, &accept_state));
        Ok(Self { address, state })
    }

    /// Where the phone connects instead of the computer.
    pub fn address(&self) -> SocketAddr {
        self.address
    }

    pub fn set(&self, faults: Faults) {
        if faults.cut_after_bytes_to_phone.is_some() {
            self.state.to_phone_bytes.store(0, Ordering::SeqCst);
        }
        *self.state.faults.lock().unwrap_or_else(PoisonError::into_inner) = faults;
        if faults.cut == Some(Direction::Both) {
            self.state.close_all();
        }
    }

    pub fn faults(&self) -> Faults {
        *self.state.faults.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Cuts every live connection both ways and refuses new ones until [`FaultyLink::heal`].
    pub fn cut(&self) {
        self.set(Faults { cut: Some(Direction::Both), ..Faults::default() });
    }

    /// No faults any more.
    pub fn heal(&self) {
        self.set(Faults::default());
    }
}

impl Drop for FaultyLink {
    fn drop(&mut self) {
        self.state.stopping.store(true, Ordering::SeqCst);
        self.state.close_all();
    }
}

impl State {
    fn faults(&self) -> Faults {
        *self.faults.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn close_all(&self) {
        for connection in self.connections.lock().unwrap_or_else(PoisonError::into_inner).drain(..) {
            let _ = connection.shutdown(Shutdown::Both);
        }
    }

    fn delay(&self, faults: &Faults) -> Duration {
        if faults.jitter.is_zero() {
            return faults.latency;
        }
        let mut seed = self.jitter_seed.load(Ordering::Relaxed);
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        self.jitter_seed.store(seed, Ordering::Relaxed);
        let span = faults.jitter.as_micros() as u64 * 2;
        let offset = Duration::from_micros(seed % span.max(1));
        (faults.latency + offset).saturating_sub(faults.jitter)
    }
}

fn accept(listener: &TcpListener, target: SocketAddr, state: &Arc<State>) {
    while !state.stopping.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((phone, _)) => {
                if state.faults().cut.is_some() {
                    let _ = phone.shutdown(Shutdown::Both);
                    continue;
                }
                let Ok(computer) = TcpStream::connect(target) else {
                    let _ = phone.shutdown(Shutdown::Both);
                    continue;
                };
                let _ = phone.set_nonblocking(false);
                for stream in [&phone, &computer] {
                    let _ = stream.set_read_timeout(Some(PUMP_POLL));
                    let _ = stream.set_nodelay(true);
                }
                {
                    let mut connections = state.connections.lock().unwrap_or_else(PoisonError::into_inner);
                    for stream in [&phone, &computer] {
                        if let Ok(clone) = stream.try_clone() {
                            connections.push(clone);
                        }
                    }
                }
                spawn_pump(state, &phone, &computer, Direction::ToComputer);
                spawn_pump(state, &computer, &phone, Direction::ToPhone);
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => thread::sleep(PUMP_POLL),
            Err(_) => thread::sleep(PUMP_POLL),
        }
    }
}

fn spawn_pump(state: &Arc<State>, from: &TcpStream, to: &TcpStream, direction: Direction) {
    let (Ok(from), Ok(to)) = (from.try_clone(), to.try_clone()) else { return };
    let state = state.clone();
    thread::spawn(move || pump(&state, from, to, direction));
}

fn pump(state: &State, mut from: TcpStream, mut to: TcpStream, direction: Direction) {
    let mut buffer = [0u8; PUMP_CHUNK];
    loop {
        if state.stopping.load(Ordering::SeqCst) {
            break;
        }
        let faults = state.faults();
        if faults.cut.is_some_and(|cut| cut == Direction::Both || cut == direction) {
            break;
        }
        if faults.stall {
            thread::sleep(PUMP_POLL);
            continue;
        }
        let read = match from.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => read,
            Err(error) if matches!(error.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut) => continue,
            Err(_) => break,
        };
        let mut bytes = &buffer[..read];
        let mut cut_now = false;
        if direction == Direction::ToPhone
            && let Some(limit) = faults.cut_after_bytes_to_phone
        {
            let sent = state.to_phone_bytes.fetch_add(read as u64, Ordering::SeqCst);
            if sent + read as u64 >= limit {
                bytes = &bytes[..usize::try_from(limit.saturating_sub(sent)).unwrap_or(0).min(read)];
                cut_now = true;
            }
        }
        let wait = state.delay(&faults) + faults.throttle.map_or(Duration::ZERO, |rate| Duration::from_secs_f64(bytes.len() as f64 / rate.max(1) as f64));
        if !wait.is_zero() {
            thread::sleep(wait);
        }
        if to.write_all(bytes).is_err() || cut_now {
            break;
        }
    }
    let _ = from.shutdown(Shutdown::Both);
    let _ = to.shutdown(Shutdown::Both);
}
