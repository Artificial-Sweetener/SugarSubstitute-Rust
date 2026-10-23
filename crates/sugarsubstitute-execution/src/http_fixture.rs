// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Scoped loopback HTTP boundary for actual native transport qualification.
use std::{
    io::{self, Read, Write},
    net::{TcpListener, TcpStream},
    time::Duration,
};
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim};

/// Exact request expectation and raw server response, including deliberately truncated bodies.
pub struct Exchange {
    /// Exact request path/query expected on this isolated fixture.
    pub path: String,
    /// Expected Range header, or its required absence.
    pub range: Option<String>,
    /// Complete raw response bytes. Closing after them can simulate a broken transfer.
    pub response: Vec<u8>,
    /// Keep a truncated response open until the client retires its socket.
    pub await_client_close: bool,
}
/// Nonblocking socket custody transferred to one execution-owned asynchronous fixture job.
pub struct Server {
    listener: TcpListener,
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
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        listener.set_nonblocking(true)?;
        Ok(Self { listener, exchanges, _memory: memory })
    }
    /// Actual already-bound port; no probe-then-bind race occurs.
    pub fn port(&self) -> io::Result<u16> {
        Ok(self.listener.local_addr()?.port())
    }
    /// Serve exactly the scripted requests, then close all sockets and release fixture custody.
    /// # Errors
    /// Rejects unexpected request bytes, credentials, peer addresses, cancellation and I/O.
    pub async fn run(self, context: JobContext) -> io::Result<()> {
        for exchange in &self.exchanges {
            let mut socket = loop {
                check(&context)?;
                match self.listener.accept() {
                    Ok((socket, peer)) if peer.ip().is_loopback() => break socket,
                    Ok(_) => return Err(io::Error::other("HTTP fixture non-loopback peer")),
                    Err(cause) if cause.kind() == io::ErrorKind::WouldBlock => {
                        wait(&context).await?
                    }
                    Err(cause) => return Err(cause),
                }
            };
            socket.set_nonblocking(true)?;
            request(&mut socket, exchange, &context).await?;
            let mut offset = 0;
            while offset < exchange.response.len() {
                check(&context)?;
                match socket.write(&exchange.response[offset..]) {
                    Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
                    Ok(count) => offset += count,
                    Err(cause) if cause.kind() == io::ErrorKind::WouldBlock => {
                        wait(&context).await?
                    }
                    Err(cause) => return Err(cause),
                }
            }
            if exchange.await_client_close {
                closed(&mut socket, &context).await?;
            }
        }
        Ok(())
    }
}
/// A silent peer exercises the client's real idle timeout without a scripted sleep.
async fn closed(socket: &mut TcpStream, context: &JobContext) -> io::Result<()> {
    let mut byte = [0];
    loop {
        check(context)?;
        match socket.read(&mut byte) {
            Ok(0) => return Ok(()),
            Ok(_) => return Err(io::Error::other("Unexpected request body")),
            Err(cause) if cause.kind() == io::ErrorKind::ConnectionReset => return Ok(()),
            Err(cause) if cause.kind() == io::ErrorKind::WouldBlock => wait(context).await?,
            Err(cause) => return Err(cause),
        }
    }
}
/// Read only bounded HTTP headers and verify the request without a general-purpose web server.
async fn request(
    socket: &mut TcpStream,
    exchange: &Exchange,
    context: &JobContext,
) -> io::Result<()> {
    let mut bytes = Vec::new();
    let mut scratch = [0; 1024];
    while !bytes.ends_with(b"\r\n\r\n") {
        check(context)?;
        match socket.read(&mut scratch) {
            Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
            Ok(count) => bytes.extend_from_slice(&scratch[..count]),
            Err(cause) if cause.kind() == io::ErrorKind::WouldBlock => wait(context).await?,
            Err(cause) => return Err(cause),
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
/// Poll nonblocking external sockets through the execution owner's cancellable timer.
async fn wait(context: &JobContext) -> io::Result<()> {
    check(context)?;
    let until = context.now().checked_add(Duration::from_millis(1)).map_err(io::Error::other)?;
    context.sleep_until(until).await;
    check(context)
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
