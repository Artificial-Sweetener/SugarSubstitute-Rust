// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own bounded installer messages inside an admitted asynchronous I/O job.
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
    Frame(String),
}

/// Native-thread endpoint. Queue operations never wait for network or worker progress.
pub struct Transport {
    commands: SyncSender<String>,
    events: Receiver<TransportEvent>,
}

/// Inert transport job input. Run only through the application's execution scope.
pub struct TransportJob {
    commands: Receiver<String>,
    events: SyncSender<TransportEvent>,
}

impl Transport {
    /// Create a fixed-capacity pair without creating a thread, socket or process.
    pub fn pair() -> (Self, TransportJob) {
        let (commands, input) = mpsc::sync_channel(QUEUE_SLOTS);
        let (output, events) = mpsc::sync_channel(QUEUE_SLOTS);
        (Self { commands, events }, TransportJob { commands: input, events: output })
    }

    /// Queue one complete JSON command, preserving caller request identity.
    /// # Errors
    /// Rejects oversized/multiline commands, closed transport and explicit overload.
    pub fn send(&self, command: String) -> io::Result<()> {
        if command.len() >= MAX_FRAME_BYTES || command.contains(['\r', '\n']) {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid provisioning frame"));
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

    /// Take at most one event without waiting. Absence is not proof of worker settlement.
    pub fn next_event(&self) -> Option<TransportEvent> {
        self.events.try_recv().ok()
    }
}

impl TransportJob {
    /// Authenticate one worker and transfer bounded messages until cancellation or disconnect.
    ///
    /// The host separately owns process-family cleanup. This job never treats a closed
    /// connection as evidence that the worker or its descendants exited.
    /// # Errors
    /// Preserves socket errors, malformed frames, failed authentication and output overload.
    pub async fn run(mut self, context: JobContext) -> io::Result<()> {
        let _memory = context.memory().claim(RESERVATION_BYTES).map_err(io::Error::other)?;
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        listener.set_nonblocking(true)?;
        let token = random_token()?;
        self.publish(
            &context,
            TransportEvent::Listening { port: listener.local_addr()?.port(), token: token.clone() },
        )
        .await?;
        let mut socket = loop {
            if stopped(&context) {
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
            if stopped(&context) {
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
                    for frame in decoder.push(&buffer[..count])? {
                        if !authenticated {
                            let value: serde_json::Value =
                                serde_json::from_str(&frame).map_err(io::Error::other)?;
                            if value["version"] != 1
                                || value["event"] != "authenticate"
                                || value["token"] != token
                            {
                                return Err(io::Error::new(
                                    io::ErrorKind::PermissionDenied,
                                    "Worker authentication failed",
                                ));
                            }
                            authenticated = true;
                        } else {
                            self.publish(&context, TransportEvent::Frame(frame)).await?;
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
