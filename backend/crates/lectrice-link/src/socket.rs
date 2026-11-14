//! Messages over the encrypted channel, and TCP set-up.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::codec;
use crate::error::{LinkError, LinkResult};
use crate::frame::{FrameReader, write_frame};
use crate::messages::Validate;
use crate::net::is_allowed_peer;
use crate::noise::{Channel, Opener, read_frame_by};

/// How long a blocking read waits before the loop looks at the clock (pings, deadlines, shutdown).
pub(crate) const POLL: Duration = Duration::from_millis(200);

pub(crate) fn send<T: Serialize>(channel: &Channel, writer: &mut impl Write, message: &T) -> LinkResult<()> {
    for frame in channel.seal(&codec::encode(message)?)? {
        write_frame(writer, &frame)?;
    }
    Ok(())
}

/// The next whole message before `deadline`.
pub(crate) fn receive_by<T: DeserializeOwned + Validate, R: Read>(opener: &mut Opener, reader: &mut FrameReader<R>, deadline: Instant) -> LinkResult<T> {
    loop {
        let frame = read_frame_by(reader, deadline)?;
        if let Some(message) = opener.open(&frame)? {
            return codec::decode(&message);
        }
    }
}

/// A TCP connection to a computer on the home network, opened by the shell's `sockets`, with the handshake's
/// timeouts set. The address rule is checked before the shell is asked.
#[cfg(feature = "client")]
pub(crate) fn connect(sockets: &dyn crate::client::SocketFactory, address: SocketAddr, timeout: Duration, allow_loopback: bool) -> LinkResult<TcpStream> {
    if !is_allowed_peer(address.ip(), allow_loopback) {
        return Err(LinkError::NotLocal);
    }
    let stream = sockets.connect(address, timeout).map_err(|error| if LinkError::is_idle(&error) { LinkError::Timeout("connect") } else { error.into() })?;
    // A factory might hand back a socket connected elsewhere; only the address that passed the rule is accepted.
    let peer = stream.peer_addr()?;
    if peer.ip().to_canonical() != address.ip().to_canonical() || peer.port() != address.port() {
        return Err(LinkError::NotLocal);
    }
    prepare(&stream, timeout)?;
    Ok(stream)
}

pub(crate) fn prepare(stream: &TcpStream, write_timeout: Duration) -> LinkResult<()> {
    stream.set_nodelay(true)?;
    stream.set_read_timeout(Some(POLL))?;
    stream.set_write_timeout(Some(write_timeout))?;
    Ok(())
}

/// The peer must be on the home network (also checked by the server before anything is read).
#[cfg(feature = "server")]
pub(crate) fn check_peer(address: SocketAddr, allow_loopback: bool) -> LinkResult<()> {
    if is_allowed_peer(address.ip(), allow_loopback) { Ok(()) } else { Err(LinkError::NotLocal) }
}

/// Pings carry the sender's own monotonic time, never a wall clock.
pub(crate) fn monotonic_ms() -> u64 {
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    u64::try_from(START.get_or_init(Instant::now).elapsed().as_millis()).unwrap_or(u64::MAX)
}
