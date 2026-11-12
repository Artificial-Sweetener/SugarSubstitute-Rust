// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Hold one client WebSocket to a loopback service inside an admitted async job.
//!
//! A consumer submits an async job whose body calls [`watch`]. The session upgrades a fresh
//! loopback TCP connection, answers pings, masks every client frame, and hands each text message
//! to the consumer's observer until the observer reports what it waited for, the watch window
//! ends, or the server closes. Binary messages and text over [`MAX_MESSAGE_BYTES`] are skipped
//! without buffering, logged, and passed to the observer as [`Incoming::Discarded`].
//!
//! Cancellation and the job deadline belong to the execution owner: when either arrives the
//! session stops reading and leaves the outcome to the job's supervisor, which drops the socket.
mod frames;
mod handshake;
#[cfg(test)]
mod tests;

use std::{io, net::SocketAddr, time::Duration};
use sugarsubstitute_contracts::execution::{JobContext, Moment};
use tokio::net::TcpStream;

/// Bytes a watch job reserves: one bounded text message, the response head and read buffers.
pub const RESERVATION_BYTES: u64 = 128 * 1024;
/// Largest text message delivered to an observer.
pub const MAX_MESSAGE_BYTES: usize = 64 * 1024;
/// Longest request path accepted for the upgrade.
const MAX_PATH_BYTES: usize = 2048;
/// Bound of the TCP connect plus the opening handshake.
const OPENING: Duration = Duration::from_secs(10);
/// Bound of one client frame write; a server that stops reading fails the session.
const WRITE: Duration = Duration::from_secs(2);

/// A validated loopback endpoint and origin-form request path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Target {
    address: SocketAddr,
    path: String,
}
impl Target {
    /// Accept a loopback `address` and an origin-form `path` such as `/ws?clientId=…`.
    /// # Errors
    /// Rejects non-loopback addresses, port zero, and paths that are empty, not starting with a
    /// single `/`, longer than 2048 bytes, or containing anything but visible ASCII.
    pub fn new(address: SocketAddr, path: &str) -> io::Result<Self> {
        if !address.ip().is_loopback() || address.port() == 0 {
            return Err(invalid("WebSocket target is not a loopback port"));
        }
        let visible = path.bytes().all(|byte| byte.is_ascii_graphic());
        if !path.starts_with('/')
            || path.starts_with("//")
            || path.len() > MAX_PATH_BYTES
            || !visible
        {
            return Err(invalid("WebSocket path is not an origin-form target"));
        }
        Ok(Self { address, path: path.to_owned() })
    }
}

/// One message the observer sees.
#[derive(Debug, Eq, PartialEq)]
pub enum Incoming<'a> {
    /// A complete text message within the bound.
    Text(&'a str),
    /// A message that was skipped: binary, or text over the bound. `bytes` is its full length.
    Discarded {
        /// Total payload bytes of the skipped message.
        bytes: u64,
        /// Whether it was binary rather than oversized text.
        binary: bool,
    },
}

/// How a watch ended without an error.
#[derive(Debug, Eq, PartialEq)]
pub enum Ended<T> {
    /// The observer returned this value.
    Observed(T),
    /// `until` arrived first; the connection was healthy throughout.
    WindowElapsed,
    /// The server closed the connection with this status code, when it sent one.
    ServerClosed(Option<u16>),
}

/// Watch `target` until `observe` returns a value, `until` arrives, or the server closes.
///
/// Call only from the body of an admitted async job whose reservation includes
/// [`RESERVATION_BYTES`]; the session claims them from `context` before connecting.
/// # Errors
/// Returns connect, handshake, protocol and write failures, an end of stream without a close,
/// and a reservation that cannot be claimed. On cancellation or the job deadline the future stays
/// pending, so the execution owner records that outcome.
pub async fn watch<T>(
    target: &Target,
    context: &JobContext,
    until: Moment,
    mut observe: impl FnMut(Incoming<'_>) -> Option<T>,
) -> io::Result<Ended<T>> {
    let _claim = context.memory().claim(RESERVATION_BYTES).map_err(io::Error::other)?;
    let socket = tokio::select! {
        opened = open(target, context) => opened?,
        () = stopped(context) => return std::future::pending().await,
    };
    let mut decoder = frames::Decoder::default();
    let mut decoded = Vec::new();
    decoder.push(&socket.1, &mut decoded)?;
    let socket = socket.0;
    let window = context.sleep_until(until);
    tokio::pin!(window);
    let mut buffer = [0; 8192];
    loop {
        for frame in decoded.drain(..) {
            match frame {
                frames::Frame::Text(text) => {
                    if let Some(value) = observe(Incoming::Text(&text)) {
                        return Ok(Ended::Observed(value));
                    }
                }
                frames::Frame::Discarded { bytes, binary } => {
                    tracing::warn!(
                        operation = "loopback_websocket_message_discarded",
                        bytes,
                        binary,
                        "A WebSocket message was not kept"
                    );
                    if let Some(value) = observe(Incoming::Discarded { bytes, binary }) {
                        return Ok(Ended::Observed(value));
                    }
                }
                frames::Frame::Ping(payload) => send(&socket, 0xA, &payload, context).await?,
                frames::Frame::Close(code) => {
                    let echo = code.map(u16::to_be_bytes);
                    send(&socket, 0x8, echo.as_ref().map_or(&[][..], |code| code), context).await?;
                    return Ok(Ended::ServerClosed(code));
                }
            }
        }
        tokio::select! {
            readiness = socket.readable() => {
                readiness?;
                match socket.try_read(&mut buffer) {
                    Ok(0) => return Err(io::Error::new(io::ErrorKind::UnexpectedEof,
                        "WebSocket connection ended without a close")),
                    Ok(count) => decoder.push(&buffer[..count], &mut decoded)?,
                    Err(cause) if cause.kind() == io::ErrorKind::WouldBlock => {}
                    Err(cause) => return Err(cause),
                }
            }
            () = &mut window => return Ok(Ended::WindowElapsed),
            () = stopped(context) => return std::future::pending().await,
        }
    }
}

/// Connect and upgrade within [`OPENING`]; returns the socket and bytes read past the head.
async fn open(target: &Target, context: &JobContext) -> io::Result<(TcpStream, Vec<u8>)> {
    let bound = context.sleep_until(context.now().checked_add(OPENING).map_err(io::Error::other)?);
    tokio::pin!(bound);
    let upgrade = async {
        let socket = TcpStream::connect(target.address).await?;
        socket.set_nodelay(true)?;
        let mut random = [0; 16];
        crate::system_random::fill(&mut random)?;
        let key = handshake::key(&random);
        let host = target.address.to_string();
        write_all(&socket, handshake::request(&host, &target.path, &key).as_bytes()).await?;
        let mut received = Vec::with_capacity(1024);
        let mut buffer = [0; 1024];
        loop {
            socket.readable().await?;
            match socket.try_read(&mut buffer) {
                Ok(0) => return Err(io::Error::from(io::ErrorKind::UnexpectedEof)),
                Ok(count) => received.extend_from_slice(&buffer[..count]),
                Err(cause) if cause.kind() == io::ErrorKind::WouldBlock => continue,
                Err(cause) => return Err(cause),
            }
            if let Some(length) = handshake::head_length(&received) {
                handshake::verify(&received[..length], &key)?;
                let rest = received.split_off(length);
                return Ok((socket, rest));
            }
            if received.len() > handshake::MAX_RESPONSE_HEAD {
                return Err(invalid("WebSocket response head is too large"));
            }
        }
    };
    tokio::select! {
        opened = upgrade => opened,
        () = &mut bound => Err(io::Error::new(io::ErrorKind::TimedOut, "WebSocket opening timed out")),
    }
}

/// Send one masked final frame within [`WRITE`].
async fn send(
    socket: &TcpStream,
    opcode: u8,
    payload: &[u8],
    context: &JobContext,
) -> io::Result<()> {
    let mut mask = [0; 4];
    crate::system_random::fill(&mut mask)?;
    let frame = frames::masked(opcode, payload, mask);
    let bound = context.sleep_until(context.now().checked_add(WRITE).map_err(io::Error::other)?);
    tokio::select! {
        written = write_all(socket, &frame) => written,
        () = bound => Err(io::Error::new(io::ErrorKind::TimedOut, "WebSocket server stopped reading")),
        () = stopped(context) => std::future::pending().await,
    }
}

async fn write_all(socket: &TcpStream, mut bytes: &[u8]) -> io::Result<()> {
    while !bytes.is_empty() {
        socket.writable().await?;
        match socket.try_write(bytes) {
            Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
            Ok(count) => bytes = &bytes[count..],
            Err(cause) if cause.kind() == io::ErrorKind::WouldBlock => {}
            Err(cause) => return Err(cause),
        }
    }
    Ok(())
}

/// Completes when the job is cancelled or its deadline passes.
async fn stopped(context: &JobContext) {
    let deadline = async {
        match context.deadline() {
            Some(deadline) => context.sleep_until(deadline).await,
            None => std::future::pending().await,
        }
    };
    tokio::select! {
        () = context.cancelled() => {}
        () = deadline => {}
    }
}

fn invalid(reason: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, reason)
}
