// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Scoped loopback HTTP boundary for actual native transport qualification.
use std::{future::Future, io, net};
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim, Scope};
use tokio::net::{TcpListener, TcpStream};

/// Exact request expectation and raw server response, including deliberately truncated bodies.
pub struct Exchange {
    /// Exact request path/query expected on this isolated fixture.
    pub path: String,
    /// Expected Range header, or its required absence.
    pub range: Option<String>,
    /// Complete raw response bytes. Closing after them can simulate a broken transfer.
    pub response: Vec<u8>,
    /// Keep the socket open until the client closes it, or reuses or replaces it for the next
    /// scripted request.
    pub await_client_close: bool,
    /// Cancel the client scope only after this peer receives its request and writes the response prefix.
    pub cancel_client: Option<std::sync::Arc<Scope>>,
}
/// Nonblocking socket custody transferred to one execution-owned asynchronous fixture job.
pub struct Server {
    listener: net::TcpListener,
    exchanges: Vec<Exchange>,
    _memory: MemoryClaim,
}
impl Server {
    /// Bind a random literal-loopback port inside an already admitted test job.
    /// # Errors
    /// Rejects excessive fixtures, exhausted admission, cancellation and native socket failures.
    pub fn bind(exchanges: Vec<Exchange>, context: &JobContext) -> io::Result<Self> {
        check(context)?;
        if exchanges.is_empty()
            || exchanges.len() > 64
            || exchanges
                .iter()
                .map(|e| e.response.len() + e.path.len() + e.range.as_ref().map_or(0, String::len))
                .sum::<usize>()
                > 4 * 1024 * 1024
        {
            return Err(io::Error::other("HTTP fixture bound"));
        }
        let memory = context.memory().claim(5 * 1024 * 1024).map_err(io::Error::other)?;
        let listener = net::TcpListener::bind(("127.0.0.1", 0))?;
        listener.set_nonblocking(true)?;
        Ok(Self { listener, exchanges, _memory: memory })
    }
    /// Actual already-bound port; no probe-then-bind race occurs.
    pub fn port(&self) -> io::Result<u16> {
        Ok(self.listener.local_addr()?.port())
    }
    /// Serve exactly the scripted requests, then close all sockets and release fixture custody.
    ///
    /// Every wait is a socket readiness event or the job's own cancellation and deadline timers, so
    /// the script advances at the pace of its peer rather than of any polling interval.
    /// # Errors
    /// Rejects unexpected request bytes, credentials, peer addresses, cancellation and I/O.
    pub async fn run(self, context: JobContext) -> io::Result<()> {
        let Self { listener, exchanges, _memory } = self;
        let listener = TcpListener::from_std(listener)?;
        // A keep-alive client may send its next request on the same socket, or pool the socket
        // and connect again; a pooled socket stays open until the script ends.
        let mut pooled = Vec::new();
        let mut next = None;
        for (index, exchange) in exchanges.iter().enumerate() {
            let (socket, prefix) = match next.take() {
                Some(next) => next,
                None => (within(&context, accept(&listener)).await?, Vec::new()),
            };
            request(&socket, exchange, prefix, &context).await?;
            let mut offset = 0;
            while offset < exchange.response.len() {
                offset +=
                    within(&context, write_some(&socket, &exchange.response[offset..])).await?;
            }
            if let Some(client) = &exchange.cancel_client {
                client.cancel();
            }
            if exchange.await_client_close {
                let more = index + 1 < exchanges.len();
                next = match closed(&socket, &listener, more, &context).await? {
                    Retirement::Closed => None,
                    Retirement::Reused(prefix) => Some((socket, prefix)),
                    Retirement::Reconnected(arrival) => {
                        pooled.push(socket);
                        Some((arrival, Vec::new()))
                    }
                };
            }
        }
        drop(pooled);
        Ok(())
    }
}
/// How the client let go of a served socket.
enum Retirement {
    Closed,
    /// The next request began on the same socket with these bytes.
    Reused(Vec<u8>),
    /// The client pooled the socket and opened this next connection.
    Reconnected(TcpStream),
}
/// What happened while the script waited for the client to let go of a served socket.
enum Arrival {
    Readable,
    Connection(TcpStream),
}
/// Accept one loopback connection, refusing any other peer.
async fn accept(listener: &TcpListener) -> io::Result<TcpStream> {
    let (socket, peer) = listener.accept().await?;
    if peer.ip().is_loopback() {
        Ok(socket)
    } else {
        Err(io::Error::other("HTTP fixture non-loopback peer"))
    }
}
/// Read the next available bytes, or none once the peer has closed its sending side.
async fn read_some(socket: &TcpStream, buffer: &mut [u8]) -> io::Result<usize> {
    loop {
        socket.readable().await?;
        match socket.try_read(buffer) {
            Err(cause) if cause.kind() == io::ErrorKind::WouldBlock => {}
            outcome => return outcome,
        }
    }
}
/// Write a non-empty prefix of `bytes`, returning how many bytes the socket accepted.
async fn write_some(socket: &TcpStream, bytes: &[u8]) -> io::Result<usize> {
    loop {
        socket.writable().await?;
        match socket.try_write(bytes) {
            Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
            Err(cause) if cause.kind() == io::ErrorKind::WouldBlock => {}
            outcome => return outcome,
        }
    }
}
/// Wait until the client closes `socket` or, while `more` requests remain scripted, reuses or
/// replaces it. A silent peer exercises the client's real idle timeout without a sleep.
async fn closed(
    socket: &TcpStream,
    listener: &TcpListener,
    more: bool,
    context: &JobContext,
) -> io::Result<Retirement> {
    let mut byte = [0];
    loop {
        let arrival = within(context, async {
            tokio::select! {
                biased;
                readable = socket.readable() => readable.map(|()| Arrival::Readable),
                connection = accept(listener), if more => connection.map(Arrival::Connection),
            }
        })
        .await?;
        match arrival {
            Arrival::Connection(next) => return Ok(Retirement::Reconnected(next)),
            Arrival::Readable => match socket.try_read(&mut byte) {
                Ok(0) => return Ok(Retirement::Closed),
                Ok(_) if more => return Ok(Retirement::Reused(byte.to_vec())),
                Ok(_) => return Err(io::Error::other("Unexpected request body")),
                Err(cause) if cause.kind() == io::ErrorKind::ConnectionReset => {
                    return Ok(Retirement::Closed);
                }
                Err(cause) if cause.kind() == io::ErrorKind::WouldBlock => {}
                Err(cause) => return Err(cause),
            },
        }
    }
}
/// Read only bounded HTTP headers, after any `prefix` already read from the same socket, and
/// verify the request without a general-purpose web server.
async fn request(
    socket: &TcpStream,
    exchange: &Exchange,
    prefix: Vec<u8>,
    context: &JobContext,
) -> io::Result<()> {
    let mut bytes = prefix;
    let mut scratch = [0; 1024];
    while !bytes.ends_with(b"\r\n\r\n") {
        match within(context, read_some(socket, &mut scratch)).await? {
            0 => return Err(io::ErrorKind::UnexpectedEof.into()),
            count => bytes.extend_from_slice(&scratch[..count]),
        }
        if bytes.len() > 8192 {
            return Err(io::Error::other("HTTP fixture request bound"));
        }
    }
    let text = std::str::from_utf8(&bytes).map_err(io::Error::other)?;
    let mut lines = text.split("\r\n");
    if lines.next() != Some(format!("GET {} HTTP/1.1", exchange.path).as_str()) {
        return Err(io::Error::other("Unexpected HTTP fixture target"));
    }
    let mut range = None;
    for line in lines.filter(|line| !line.is_empty()) {
        let (name, value) =
            line.split_once(':').ok_or_else(|| io::Error::other("Malformed fixture request"))?;
        if matches!(
            name.to_ascii_lowercase().as_str(),
            "authorization" | "proxy-authorization" | "cookie"
        ) {
            return Err(io::Error::other("Ambient HTTP credentials leaked"));
        }
        if name.eq_ignore_ascii_case("range") {
            if range.is_some() {
                return Err(io::Error::other("Duplicate Range request"));
            }
            range = Some(value.trim());
        }
    }
    if range != exchange.range.as_deref() {
        return Err(io::Error::other("Unexpected HTTP fixture range"));
    }
    Ok(())
}
/// Let `work` finish only while the job is live: cancellation and the job's own deadline end the
/// wait through the execution owner's timers, never through elapsed polling.
async fn within<T>(
    context: &JobContext,
    work: impl Future<Output = io::Result<T>>,
) -> io::Result<T> {
    check(context)?;
    let deadline = context.deadline().ok_or(io::ErrorKind::TimedOut)?;
    tokio::select! {
        biased;
        outcome = work => outcome,
        () = context.cancelled() => Err(io::ErrorKind::Interrupted.into()),
        () = context.sleep_until(deadline) => Err(io::ErrorKind::TimedOut.into()),
    }
}
/// Fixture lifetimes are always finite, even when a client never connects.
fn check(context: &JobContext) -> io::Result<()> {
    if context.cancellation_requested() {
        return Err(io::ErrorKind::Interrupted.into());
    }
    if context.deadline().is_none_or(|end| context.now() >= end) {
        return Err(io::ErrorKind::TimedOut.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests;
