// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own bounded installer messages inside an admitted asynchronous I/O job.
mod memory;
#[cfg(windows)]
#[allow(unsafe_code)]
mod random;
mod wire;

use std::{
    io::{self, Read, Write},
    net::{Shutdown, TcpListener, TcpStream},
    sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError},
    time::{Duration, Instant},
};
use sugarsubstitute_contracts::execution::JobContext;

/// Peak protocol buffers, queues and retained events reserved before worker admission.
pub const RESERVATION_BYTES: u64 = 4 * 1024 * 1024;
/// Maximum serialized command or event, including its newline.
pub const MAX_FRAME_BYTES: usize = 65_536;
const QUEUE_SLOTS: usize = 16;

/// Observable transport events; endpoint custody remains with the admitted job.
pub enum TransportEvent {
    /// Loopback listener is live; launch exactly one worker with this capability.
    Listening {
        /// Already bound loopback port.
        port: u16,
        /// Single-use random connection credential; never log this value.
        token: String,
    },
    /// One complete bounded UTF-8 JSON event from the authenticated worker.
    Frame(Frame),
}

/// A received UTF-8 message retaining original admission and one of sixteen delivery slots.
///
/// Borrow its bytes for decoding. Drop the frame after decoding to permit another delivery.
/// Holding a frame after transport completion retains its original memory reservation.
pub struct Frame {
    bytes: String,
    _permit: memory::Permit,
}
impl Frame {
    /// Borrow the complete message without its newline; no writable allocation escapes.
    pub fn as_str(&self) -> &str {
        &self.bytes
    }
}

/// Native-thread endpoint. Queue operations never wait for network or worker progress.
pub struct Transport {
    commands: SyncSender<String>,
    events: Receiver<TransportEvent>,
    budget: memory::Budget,
}

/// Inert transport job input. Run only through the application's execution scope.
pub struct TransportJob {
    commands: Receiver<String>,
    events: SyncSender<TransportEvent>,
    budget: memory::Budget,
}

impl Transport {
    /// Create a fixed-capacity pair without creating a thread, socket or process.
    pub fn pair() -> (Self, TransportJob) {
        let (commands, input) = mpsc::sync_channel(QUEUE_SLOTS);
        let (output, events) = mpsc::sync_channel(QUEUE_SLOTS);
        let budget = memory::Budget::new();
        (
            Self { commands, events, budget: budget.clone() },
            TransportJob { commands: input, events: output, budget },
        )
    }

    /// Queue one complete JSON command, preserving caller request identity.
    /// # Errors
    /// Rejects oversized/multiline commands, allocations exceeding the frame bound,
    /// commands before job admission, closed transport and explicit overload.
    /// Wait for `Listening` before sending commands.
    pub fn send(&self, command: String) -> io::Result<()> {
        if command.len() >= MAX_FRAME_BYTES
            || command.capacity() > MAX_FRAME_BYTES
            || command.contains(['\r', '\n'])
        {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid provisioning frame"));
        }
        if !self.budget.admitted() {
            return Err(io::Error::new(io::ErrorKind::WouldBlock, "Transport not admitted"));
        }
        self.commands.try_send(command).map_err(|cause| match cause {
            TrySendError::Full(_) => {
                io::Error::new(io::ErrorKind::WouldBlock, "Provisioning command queue full")
            }
            TrySendError::Disconnected(_) => {
                io::Error::new(io::ErrorKind::BrokenPipe, "Provisioning transport closed")
            }
        })
    }

    /// Take at most one event without waiting. Received frames keep their delivery slots
    /// until dropped; decoding must finish before another sixteen frames can arrive.
    /// Absence is not proof of worker settlement.
    pub fn next_event(&self) -> Option<TransportEvent> {
        self.events.try_recv().ok()
    }
}

impl Drop for Transport {
    fn drop(&mut self) {
        self.budget.close();
    }
}

impl TransportJob {
    /// Authenticate one worker and transfer bounded messages until cancellation or disconnect.
    ///
    /// The host separately owns process-family cleanup. This job never treats a closed
    /// connection as evidence that the worker or its descendants exited.
    /// # Errors
    /// Preserves socket errors, malformed frames, failed authentication and output overload.
    /// Admitted failures retain original memory custody, including rejected input bytes.
    pub async fn run(self, context: JobContext) -> io::Result<()> {
        self.budget.activate(context.memory())?;
        let budget = self.budget.clone();
        self.serve(context).await.map_err(|cause| budget.retain_failure(cause))
    }

    /// Run the socket lifecycle while the outer result owner retains failure custody.
    async fn serve(mut self, context: JobContext) -> io::Result<()> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        listener.set_nonblocking(true)?;
        let token = random_token()?;
        self.publish(
            &context,
            TransportEvent::Listening { port: listener.local_addr()?.port(), token: token.clone() },
        )
        .await?;
        let mut socket = loop {
            if stopped(&context) || self.budget.closed() {
                return Ok(());
            }
            match listener.accept() {
                Ok((socket, address)) if address.ip().is_loopback() => break socket,
                Ok(_) => {
                    return Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "Non-loopback worker",
                    ));
                }
                Err(cause) if cause.kind() == io::ErrorKind::WouldBlock => {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
                Err(cause) => return Err(cause),
            }
        };
        drop(listener);
        socket.set_nonblocking(true)?;
        let authenticated_by = Instant::now() + Duration::from_secs(15);
        let mut decoder = wire::Decoder::default();
        let mut authenticated = false;
        let mut buffer = [0; 8192];
        loop {
            if stopped(&context) || self.budget.closed() {
                socket.shutdown(Shutdown::Both)?;
                return Ok(());
            }
            if !authenticated && Instant::now() >= authenticated_by {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "Worker authentication timed out",
                ));
            }
            if authenticated && !self.send_commands(&mut socket, &context).await? {
                return Ok(());
            }
            match socket.read(&mut buffer) {
                Ok(0) => return decoder.finish(),
                Ok(count) => {
                    for &byte in &buffer[..count] {
                        let Some(frame) = decoder.push(byte)? else { continue };
                        if !authenticated {
                            wire::authenticate(&frame, &token)?;
                            authenticated = true;
                        } else {
                            let permit = loop {
                                if stopped(&context) {
                                    return Ok(());
                                }
                                match self.budget.permit() {
                                    Ok(permit) => break permit,
                                    Err(cause) if cause.kind() == io::ErrorKind::WouldBlock => {
                                        tokio::time::sleep(Duration::from_millis(10)).await;
                                    }
                                    Err(cause) => return Err(cause),
                                }
                            };
                            self.publish(
                                &context,
                                TransportEvent::Frame(Frame { bytes: frame, _permit: permit }),
                            )
                            .await?;
                        }
                    }
                }
                Err(cause)
                    if matches!(
                        cause.kind(),
                        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
                    ) =>
                {
                    tokio::time::sleep(Duration::from_millis(10)).await
                }
                Err(cause) => return Err(cause),
            }
        }
    }

    async fn send_commands(
        &mut self,
        socket: &mut TcpStream,
        context: &JobContext,
    ) -> io::Result<bool> {
        for _ in 0..QUEUE_SLOTS {
            match self.commands.try_recv() {
                Ok(mut command) => {
                    command.push('\n');
                    let mut remaining = command.as_bytes();
                    let until = Instant::now() + Duration::from_secs(2);
                    while !remaining.is_empty() {
                        if stopped(context) {
                            return Ok(false);
                        }
                        match socket.write(remaining) {
                            Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
                            Ok(count) => remaining = &remaining[count..],
                            Err(cause) if cause.kind() == io::ErrorKind::WouldBlock => {
                                if Instant::now() >= until {
                                    return Err(io::Error::new(
                                        io::ErrorKind::TimedOut,
                                        "Worker is not reading commands",
                                    ));
                                }
                                tokio::time::sleep(Duration::from_millis(10)).await;
                            }
                            Err(cause) => return Err(cause),
                        }
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    socket.shutdown(Shutdown::Both)?;
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }

    async fn publish(&mut self, context: &JobContext, mut event: TransportEvent) -> io::Result<()> {
        loop {
            if stopped(context) {
                return Ok(());
            }
            match self.events.try_send(event) {
                Ok(()) => return Ok(()),
                Err(TrySendError::Full(retained)) => {
                    event = retained;
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
                Err(TrySendError::Disconnected(_)) => {
                    return Err(io::Error::new(io::ErrorKind::BrokenPipe, "Installer closed"));
                }
            }
        }
    }
}

fn stopped(context: &JobContext) -> bool {
    context.cancellation_requested()
        || context.deadline().is_some_and(|until| context.now() >= until)
}

fn random_token() -> io::Result<String> {
    let mut bytes = [0; 32];
    #[cfg(windows)]
    random::fill(&mut bytes)?;
    #[cfg(not(windows))]
    std::fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

#[cfg(test)]
mod tests;
