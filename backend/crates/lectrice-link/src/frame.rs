//! Transport framing: `u16 length (big-endian) ‖ bytes`. The reader keeps a partial frame across read timeouts, so a
//! socket with a short read timeout can wake up to send pings without losing its place.

use std::io::{self, Read, Write};

use crate::error::LinkError;

const LENGTH_BYTES: usize = 2;
const READ_CHUNK: usize = 16 * 1024;

pub(crate) fn write_frame(writer: &mut impl Write, payload: &[u8]) -> io::Result<()> {
    let length = u16::try_from(payload.len()).map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "frame too long"))?;
    let mut bytes = Vec::with_capacity(LENGTH_BYTES + payload.len());
    bytes.extend_from_slice(&length.to_be_bytes());
    bytes.extend_from_slice(payload);
    writer.write_all(&bytes)?;
    writer.flush()
}

/// Splits a byte stream into frames.
pub struct FrameReader<R> {
    inner: R,
    buffer: Vec<u8>,
    progressed: bool,
}

impl<R: Read> FrameReader<R> {
    pub fn new(inner: R) -> Self {
        Self { inner, buffer: Vec::with_capacity(READ_CHUNK), progressed: false }
    }

    #[cfg(test)]
    pub fn get_ref(&self) -> &R {
        &self.inner
    }

    /// `Ok(None)` when the read timed out before a whole frame arrived (the partial frame is kept); an error when the
    /// stream ended or broke.
    pub fn read_frame(&mut self) -> io::Result<Option<Vec<u8>>> {
        loop {
            if let Some(frame) = self.take_frame() {
                return Ok(Some(frame));
            }
            let mut chunk = [0u8; READ_CHUNK];
            match self.inner.read(&mut chunk) {
                Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
                Ok(read) => {
                    self.buffer.extend_from_slice(&chunk[..read]);
                    self.progressed = true;
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) if LinkError::is_idle(&error) => return Ok(None),
                Err(error) => return Err(error),
            }
        }
    }

    /// True when any bytes arrived since the last call: a slow link that is still moving a long frame is alive.
    pub fn take_progress(&mut self) -> bool {
        std::mem::take(&mut self.progressed)
    }

    fn take_frame(&mut self) -> Option<Vec<u8>> {
        let header: [u8; LENGTH_BYTES] = self.buffer.get(..LENGTH_BYTES)?.try_into().ok()?;
        let length = usize::from(u16::from_be_bytes(header));
        if self.buffer.len() < LENGTH_BYTES + length {
            return None;
        }
        let frame = self.buffer[LENGTH_BYTES..LENGTH_BYTES + length].to_vec();
        self.buffer.drain(..LENGTH_BYTES + length);
        Some(frame)
    }
}

/// Splits `bytes` into whole frames (a fuzz target: any input either yields frames or leaves a partial tail).
pub fn split_frames(bytes: &[u8]) -> (Vec<Vec<u8>>, usize) {
    let mut reader = FrameReader::new(bytes);
    let mut frames = Vec::new();
    while let Ok(Some(frame)) = reader.read_frame() {
        frames.push(frame);
    }
    let tail = reader.buffer.len();
    (frames, tail)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::limits::MAX_FRAME_BYTES;

    /// Gives one byte per read and a timeout after each, like a slow socket.
    struct Trickle {
        bytes: Vec<u8>,
        at: usize,
        timed_out: bool,
    }

    impl Read for Trickle {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.timed_out = !self.timed_out;
            if self.timed_out {
                return Err(io::ErrorKind::WouldBlock.into());
            }
            let Some(byte) = self.bytes.get(self.at) else { return Ok(0) };
            buf[0] = *byte;
            self.at += 1;
            Ok(1)
        }
    }

    #[test]
    fn round_trips_frames_including_empty_and_maximal() {
        let payloads = [Vec::new(), vec![7u8; 3], vec![1u8; MAX_FRAME_BYTES]];
        let mut wire = Vec::new();
        for payload in &payloads {
            write_frame(&mut wire, payload).unwrap();
        }
        let (frames, tail) = split_frames(&wire);
        assert_eq!(frames, payloads);
        assert_eq!(tail, 0);
    }

    #[test]
    fn refuses_a_frame_over_the_limit() {
        assert!(write_frame(&mut Vec::new(), &vec![0u8; MAX_FRAME_BYTES + 1]).is_err());
    }

    #[test]
    fn keeps_a_partial_frame_across_timeouts() {
        let mut wire = Vec::new();
        write_frame(&mut wire, b"hello").unwrap();
        let mut reader = FrameReader::new(Trickle { bytes: wire, at: 0, timed_out: false });
        let mut timeouts = 0;
        let frame = loop {
            match reader.read_frame().unwrap() {
                Some(frame) => break frame,
                None => timeouts += 1,
            }
        };
        assert_eq!(frame, b"hello");
        assert!(timeouts >= 6);
        let ended = (0..4).map(|_| reader.read_frame()).any(|result| result.is_err());
        assert!(ended, "the stream ended");
    }

    #[test]
    fn a_cut_mid_frame_is_an_error_not_a_frame() {
        let mut wire = Vec::new();
        write_frame(&mut wire, &[9u8; 100]).unwrap();
        wire.truncate(50);
        let mut reader = FrameReader::new(&wire[..]);
        assert_eq!(reader.read_frame().unwrap_err().kind(), io::ErrorKind::UnexpectedEof);
    }
}
