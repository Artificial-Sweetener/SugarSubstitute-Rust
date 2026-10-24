// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own bounded installer messages inside an admitted asynchronous I/O job.
mod memory;
mod native;
mod socket;
pub use native::DownloadPort;
#[cfg(windows)]
#[allow(unsafe_code)]
mod random;
mod wire;

#[cfg(not(windows))]
use std::io::Read;
use std::{
    io,
    sync::mpsc::{self, Receiver, SyncSender},
};
use tokio::sync::mpsc::{Receiver as CommandReceiver, Sender};

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
    commands: Sender<String>,
    events: Receiver<TransportEvent>,
    downloads: Option<Receiver<Frame>>,
    budget: memory::Budget,
}

/// Inert transport job input. Run only through the application's execution scope.
pub struct TransportJob {
    commands: CommandReceiver<String>,
    events: SyncSender<TransportEvent>,
    downloads: SyncSender<Frame>,
    budget: memory::Budget,
}

impl Transport {
    /// Create a fixed-capacity pair without creating a thread, socket or process.
    pub fn pair() -> (Self, TransportJob) {
        let (commands, input) = tokio::sync::mpsc::channel(QUEUE_SLOTS);
        let (output, events) = mpsc::sync_channel(QUEUE_SLOTS);
        let (downloads, native) = mpsc::sync_channel(QUEUE_SLOTS);
        let budget = memory::Budget::new();
        (
            Self { commands, events, downloads: Some(native), budget: budget.clone() },
            TransportJob { commands: input, events: output, downloads, budget },
        )
    }

    /// Queue one complete JSON command, preserving caller request identity.
    /// # Errors
    /// Rejects oversized/multiline commands, allocations exceeding the frame bound,
    /// commands before job admission, closed transport and explicit overload.
    /// Wait for `Listening` before sending commands.
    pub fn send(&self, command: String) -> io::Result<()> {
        send(&self.commands, &self.budget, command)
    }

    /// Move the authenticated download lane to one execution-owned native storage service.
    /// The lane shares the original sixteen delivery slots and four MiB transport reservation.
    /// # Errors
    /// Rejects a second consumer. Taking this port creates neither a worker nor native I/O.
    pub fn take_downloads(&mut self) -> io::Result<DownloadPort> {
        let input =
            self.downloads.take().ok_or_else(|| io::Error::other("Download lane already owned"))?;
        Ok(DownloadPort { input, commands: self.commands.clone(), budget: self.budget.clone() })
    }

    /// Take at most one event without waiting. Received frames keep their delivery slots
    /// until dropped; decoding must finish before another sixteen frames can arrive.
    /// Absence is not proof of worker settlement.
    pub fn next_event(&self) -> Option<TransportEvent> {
        let event = self.events.try_recv().ok();
        if event.is_some() {
            self.budget.signal();
        }
        event
    }
}

impl Drop for Transport {
    fn drop(&mut self) {
        self.budget.close();
    }
}

/// Queue only a bounded allocation after actual transport admission, using one shared command rule.
fn send(commands: &Sender<String>, budget: &memory::Budget, command: String) -> io::Result<()> {
    if command.len() >= MAX_FRAME_BYTES
        || command.capacity() > MAX_FRAME_BYTES
        || command.contains(['\r', '\n'])
    {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid provisioning frame"));
    }
    if !budget.admitted() {
        return Err(io::Error::new(io::ErrorKind::WouldBlock, "Transport not admitted"));
    }
    if budget.closed() {
        return Err(io::Error::new(io::ErrorKind::BrokenPipe, "Provisioning transport closed"));
    }
    commands.try_send(command).map_err(|cause| match cause {
        tokio::sync::mpsc::error::TrySendError::Full(_) => {
            io::Error::new(io::ErrorKind::WouldBlock, "Provisioning command queue full")
        }
        tokio::sync::mpsc::error::TrySendError::Closed(_) => {
            io::Error::new(io::ErrorKind::BrokenPipe, "Provisioning transport closed")
        }
    })
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
mod native_tests;
#[cfg(test)]
mod tests;
