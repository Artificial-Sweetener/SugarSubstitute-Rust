// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Drive authenticated worker sockets from readiness, commands and owned cancellation.
use super::*;
use std::{net::Shutdown, time::Duration};
use sugarsubstitute_contracts::execution::JobContext;
use tokio::net::{TcpListener, TcpStream};
#[cfg(test)]
mod tests;

impl TransportJob {
    /// Authenticate one worker and retain every admitted frame and failure through actual delivery.
    /// Process-family retirement remains independently owned by the native process registry.
    /// Submit through the execution adapter: cancellation and deadline waits stay pending until
    /// that original supervisor classifies the outcome and drops the owned socket future.
    /// # Errors
    /// Preserves framing, authentication, socket and capacity errors with original memory custody.
    pub async fn run(self, context: JobContext) -> io::Result<()> {
        self.budget.activate(context.memory())?;
        let budget = self.budget.clone();
        self.serve(context).await.map_err(|cause| budget.retain_failure(cause))
    }

    /// Bind before launch and accept exactly one authenticated loopback connection.
    async fn serve(mut self, context: JobContext) -> io::Result<()> {
        if controls_ended(&context) {
            return std::future::pending().await;
        }
        let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
        let token = random_token()?;
        self.publish(
            &context,
            TransportEvent::Listening { port: listener.local_addr()?.port(), token: token.clone() },
        )
        .await?;
        let accepted = tokio::select! {
            accepted = listener.accept() => accepted?,
            stopped = stop(&context, &self.budget) => { stopped?; return Ok(()); }
        };
        drop(listener);
        let (socket, address) = accepted;
        socket.set_nodelay(true)?;
        if !address.ip().is_loopback() {
            return Err(io::Error::new(io::ErrorKind::PermissionDenied, "Non-loopback worker"));
        }
        if self.transfer(&socket, &token, &context).await? {
            socket.into_std()?.shutdown(Shutdown::Both)?;
        }
        Ok(())
    }

    /// Select actual readiness rather than adding a polling delay to every native storage reply.
    async fn transfer(
        &mut self,
        socket: &TcpStream,
        token: &str,
        context: &JobContext,
    ) -> io::Result<bool> {
        let authentication = context.sleep_until(
            context.now().checked_add(Duration::from_secs(15)).map_err(io::Error::other)?,
        );
        tokio::pin!(authentication);
        let mut authenticated = false;
        let mut decoder = wire::Decoder::default();
        let mut buffer = [0; 8192];
        loop {
            tokio::select! {
                readiness = socket.readable() => {
                    readiness?;
                    match socket.try_read(&mut buffer) {
                        Ok(0) => { decoder.finish()?; return Ok(false); }
                        Ok(count) => for &byte in &buffer[..count] {
                            let Some(frame) = decoder.push(byte)? else { continue };
                            if authenticated {
                                self.deliver(context, frame).await?;
                            } else {
                                wire::authenticate(&frame, token)?;
                                authenticated = true;
                            }
                        },
                        Err(cause) if cause.kind() == io::ErrorKind::WouldBlock => {},
                        Err(cause) => return Err(cause),
                    }
                }
                command = self.commands.recv(), if authenticated => {
                    let Some(command) = command else { return Ok(true) };
                    write(socket, command, context, &self.budget).await?;
                }
                _ = &mut authentication, if !authenticated => {
                    return Err(io::Error::new(io::ErrorKind::TimedOut, "Worker authentication timed out"));
                }
                stopped = stop(context, &self.budget) => { stopped?; return Ok(true); }
            }
        }
    }

    /// A native frame bypasses UI polling while sharing the same total retained-message limit.
    async fn deliver(&mut self, context: &JobContext, bytes: String) -> io::Result<()> {
        let native = wire::is_download(&bytes)?;
        let permit = loop {
            let changed = self.budget.notified()?;
            tokio::pin!(changed);
            changed.as_mut().enable();
            if controls_ended(context) {
                return std::future::pending().await;
            }
            match self.budget.permit() {
                Ok(permit) => break permit,
                Err(cause) if cause.kind() == io::ErrorKind::WouldBlock => {
                    tokio::select! {
                        _ = &mut changed => {},
                        _ = context.cancelled() => return std::future::pending().await,
                        _ = deadline(context) => return std::future::pending().await,
                    }
                }
                Err(cause) => return Err(cause),
            }
        };
        let frame = Frame { bytes, _permit: permit };
        if native {
            self.budget.native_offered()?;
            if let Err(cause) = self.downloads.try_send(frame) {
                self.budget.native_taken();
                return Err(match cause {
                    mpsc::TrySendError::Full(_) => {
                        io::Error::new(io::ErrorKind::WouldBlock, "Native download queue full")
                    }
                    mpsc::TrySendError::Disconnected(_) => {
                        io::Error::new(io::ErrorKind::BrokenPipe, "Native download receiver closed")
                    }
                });
            }
            Ok(())
        } else {
            self.publish(context, TransportEvent::Frame(frame)).await
        }
    }

    /// Register before checking the fixed UI queue; queue and permit releases wake this owner.
    async fn publish(&mut self, context: &JobContext, mut event: TransportEvent) -> io::Result<()> {
        loop {
            let changed = self.budget.notified()?;
            tokio::pin!(changed);
            changed.as_mut().enable();
            if controls_ended(context) {
                return std::future::pending().await;
            }
            match self.events.try_send(event) {
                Ok(()) => return Ok(()),
                Err(mpsc::TrySendError::Full(retained)) => event = retained,
                Err(mpsc::TrySendError::Disconnected(_)) => {
                    return Err(io::ErrorKind::BrokenPipe.into());
                }
            }
            tokio::select! {
                _ = &mut changed => {},
                _ = context.cancelled() => return std::future::pending().await,
                _ = deadline(context) => return std::future::pending().await,
            }
        }
    }
}

/// Bound an entire command's transmission without restarting its timeout after partial writes.
async fn write(
    socket: &TcpStream,
    mut command: String,
    context: &JobContext,
    budget: &memory::Budget,
) -> io::Result<()> {
    command.push('\n');
    let mut bytes = command.as_bytes();
    let timeout = context
        .sleep_until(context.now().checked_add(Duration::from_secs(2)).map_err(io::Error::other)?);
    tokio::pin!(timeout);
    while !bytes.is_empty() {
        if controls_ended(context) {
            return std::future::pending().await;
        }
        tokio::select! {
            readiness = socket.writable() => {
                readiness?;
                match socket.try_write(bytes) {
                    Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
                    Ok(count) => bytes = &bytes[count..],
                    Err(cause) if cause.kind() == io::ErrorKind::WouldBlock => {},
                    Err(cause) => return Err(cause),
                }
            }
            _ = &mut timeout => return Err(io::Error::new(io::ErrorKind::TimedOut, "Worker is not reading commands")),
            stopped = stop(context, budget) => { stopped?; return Err(io::ErrorKind::Interrupted.into()); }
        }
    }
    Ok(())
}

/// Observe the current job's actual clock and parent deadline without creating another timer owner.
async fn deadline(context: &JobContext) {
    match context.deadline() {
        Some(deadline) => context.sleep_until(deadline).await,
        None => std::future::pending::<()>().await,
    }
}

/// Observe original controls before a simultaneously closed endpoint can publish success.
fn controls_ended(context: &JobContext) -> bool {
    context.cancellation_requested()
        || context.deadline().is_some_and(|until| context.now() >= until)
}

/// Host closure completes cleanup; cancellation/deadlines are classified by the original supervisor.
async fn stop(context: &JobContext, budget: &memory::Budget) -> io::Result<()> {
    loop {
        let changed = budget.notified()?;
        tokio::pin!(changed);
        changed.as_mut().enable();
        if controls_ended(context) {
            return std::future::pending().await;
        }
        if budget.closed() {
            return Ok(());
        }
        tokio::select! {
            _ = &mut changed => {},
            _ = context.cancelled() => return std::future::pending().await,
            _ = deadline(context) => return std::future::pending().await,
        }
    }
}
