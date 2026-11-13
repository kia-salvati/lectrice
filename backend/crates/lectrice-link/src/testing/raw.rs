//! Peers that speak the protocol by hand (feature `testing`), for the malicious-peer tests of this crate and of both
//! shells: a phone that floods, stalls, never reads or sends garbage, and a computer that answers with whatever the
//! test says. They do the real handshakes, then send messages without the checks and manners the real client and
//! server have. For tests only: no address rule, no reconnect, no pings.

use std::io::{self, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::time::{Duration, Instant};

use snow::Builder;

use crate::error::{LinkError, LinkResult};
use crate::frame::{FrameReader, write_frame};
use crate::identity::{KEY_BYTES, StaticKeys};
use crate::limits::{PROTOCOL_MINOR, SESSION_PROLOGUE};
use crate::messages::{ClientMessage, Hello, ServerMessage, Welcome};
use crate::noise::{Channel, IK_PARAMS, Opener, params, receive_handshake, send_handshake};
use crate::socket::{POLL, receive_by, send};

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);

struct Wire {
    stream: TcpStream,
    reader: FrameReader<TcpStream>,
    channel: Channel,
    opener: Opener,
}

impl Wire {
    fn new(stream: TcpStream, reader: FrameReader<TcpStream>, channel: Channel) -> Self {
        let opener = channel.opener();
        Self { stream, reader, channel, opener }
    }
}

fn prepared(stream: &TcpStream) -> io::Result<()> {
    stream.set_nodelay(true)?;
    stream.set_read_timeout(Some(POLL))
}

/// A phone under the test's control.
pub struct RawPhone {
    wire: Wire,
    /// The first handshake message as it went over the wire, for replay tests.
    first_message: Vec<u8>,
}

impl RawPhone {
    /// TCP, the session prologue and the Noise `IK` handshake with `keys`; no `Hello` yet.
    pub fn handshake(address: SocketAddr, keys: &StaticKeys, computer_key: &[u8; KEY_BYTES]) -> LinkResult<Self> {
        let mut stream = TcpStream::connect_timeout(&address, HANDSHAKE_TIMEOUT)?;
        prepared(&stream)?;
        stream.write_all(SESSION_PROLOGUE)?;
        let mut reader = FrameReader::new(stream.try_clone()?);
        let mut state = Builder::new(params(IK_PARAMS))
            .local_private_key(keys.private())?
            .remote_public_key(computer_key)?
            .prologue(SESSION_PROLOGUE)?
            .build_initiator()?;
        let mut first_message = Vec::new();
        send_handshake(&mut state, &mut first_message, &[])?;
        stream.write_all(&first_message)?;
        receive_handshake(&mut state, &mut reader, Instant::now() + HANDSHAKE_TIMEOUT)?;
        let channel = Channel::new(state.into_transport_mode()?);
        Ok(Self { wire: Wire::new(stream, reader, channel), first_message })
    }

    /// [`Self::handshake`], then `Hello`; returns the computer's first answer (`Welcome`, or `Bye`).
    pub fn connect(address: SocketAddr, keys: &StaticKeys, computer_key: &[u8; KEY_BYTES], device_name: &str) -> LinkResult<(Self, ServerMessage)> {
        let mut phone = Self::handshake(address, keys, computer_key)?;
        phone.send(&ClientMessage::Hello(Hello { proto_minor: PROTOCOL_MINOR, app_version: "0.0.0-test".into(), device_name: device_name.into() }))?;
        let answer = phone.receive(HANDSHAKE_TIMEOUT)?;
        Ok((phone, answer))
    }

    /// The framed first handshake message this phone sent (send it again on a new connection to replay it).
    pub fn first_handshake_frame(&self) -> &[u8] {
        &self.first_message
    }

    /// Encrypts and sends `message` as it is: nothing is checked on the way out.
    pub fn send(&mut self, message: &ClientMessage) -> LinkResult<()> {
        send(&self.wire.channel, &mut self.wire.stream, message)
    }

    /// Sends bytes inside the encrypted channel that needn't be a message at all.
    pub fn send_plaintext(&mut self, bytes: &[u8]) -> LinkResult<()> {
        for frame in self.wire.channel.seal(bytes)? {
            write_frame(&mut self.wire.stream, &frame)?;
        }
        Ok(())
    }

    /// Sends one frame of raw bytes (not encrypted): to the computer this is a forged or damaged frame.
    pub fn send_raw_frame(&mut self, bytes: &[u8]) -> io::Result<()> {
        write_frame(&mut self.wire.stream, bytes)
    }

    /// The computer's next message within `timeout` (as decoded and checked by the phone's own rules).
    pub fn receive(&mut self, timeout: Duration) -> LinkResult<ServerMessage> {
        receive_by(&mut self.wire.opener, &mut self.wire.reader, Instant::now() + timeout)
    }

    /// True once the computer closed the connection (nothing more to read), within `timeout`.
    pub fn is_closed_within(&mut self, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            match self.receive(deadline.saturating_duration_since(Instant::now())) {
                Ok(_) | Err(LinkError::Timeout(_)) => {}
                Err(_) => return true,
            }
        }
        false
    }

    pub fn stream(&self) -> &TcpStream {
        &self.wire.stream
    }
}

/// A computer under the test's control, on loopback.
pub struct RawComputer {
    listener: TcpListener,
    keys: StaticKeys,
}

impl RawComputer {
    pub fn bind(keys: StaticKeys) -> io::Result<Self> {
        Ok(Self { listener: TcpListener::bind("127.0.0.1:0")?, keys })
    }

    pub fn address(&self) -> io::Result<SocketAddr> {
        self.listener.local_addr()
    }

    /// Accepts one phone: prologue, Noise `IK` (any phone key), its `Hello`, then `welcome` as the answer.
    pub fn accept(&self, welcome: Welcome) -> LinkResult<RawComputerSession> {
        let (stream, _) = self.listener.accept()?;
        prepared(&stream)?;
        let mut writer = stream.try_clone()?;
        let mut reader = FrameReader::new(stream.try_clone()?);
        let deadline = Instant::now() + HANDSHAKE_TIMEOUT;
        read_prologue(&stream, deadline)?;
        let mut state = Builder::new(params(IK_PARAMS)).local_private_key(self.keys.private())?.prologue(SESSION_PROLOGUE)?.build_responder()?;
        receive_handshake(&mut state, &mut reader, deadline)?;
        send_handshake(&mut state, &mut writer, &[])?;
        let mut session = RawComputerSession { wire: Wire::new(stream, reader, Channel::new(state.into_transport_mode()?)) };
        match session.receive(HANDSHAKE_TIMEOUT)? {
            ClientMessage::Hello(_) => {}
            _ => return Err(LinkError::Protocol("expected Hello")),
        }
        session.send(&ServerMessage::Welcome(welcome))?;
        Ok(session)
    }
}

fn read_prologue(mut stream: &TcpStream, deadline: Instant) -> LinkResult<()> {
    use std::io::Read;
    let mut line = Vec::new();
    let mut byte = [0u8; 1];
    while line.len() < SESSION_PROLOGUE.len() {
        match stream.read(&mut byte) {
            Ok(0) => return Err(LinkError::Disconnected),
            Ok(_) => line.push(byte[0]),
            Err(error) if LinkError::is_idle(&error) && Instant::now() < deadline => {}
            Err(error) => return Err(error.into()),
        }
    }
    if line == SESSION_PROLOGUE { Ok(()) } else { Err(LinkError::Incompatible("prologue")) }
}

pub struct RawComputerSession {
    wire: Wire,
}

impl RawComputerSession {
    /// Encrypts and sends `message` as it is: nothing is checked on the way out.
    pub fn send(&mut self, message: &ServerMessage) -> LinkResult<()> {
        send(&self.wire.channel, &mut self.wire.stream, message)
    }

    pub fn receive(&mut self, timeout: Duration) -> LinkResult<ClientMessage> {
        receive_by(&mut self.wire.opener, &mut self.wire.reader, Instant::now() + timeout)
    }

    pub fn close(self) {
        let _ = self.wire.stream.shutdown(Shutdown::Both);
    }
}
