// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! One private request/reply exchange with retained, nonblocking native I/O.
mod native;
mod operation;
mod pair;
#[cfg(test)]
mod tests;
pub use crate::native::NativeObservationError;
use std::sync::{Arc, Mutex};
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim};
mod handle;
use handle::Handle;

/// Maximum logical frame size before any peer-controlled allocation.
pub const MAX_FRAME_BYTES: usize = 2 * 1024 * 1024;

/// Reject malformed or closed transport state without native effects.
fn invalid(stage: &'static str) -> NativeObservationError {
    NativeObservationError::new(stage, std::io::Error::from(std::io::ErrorKind::InvalidData))
}

/// Require the same finite admitted worker lifetime for every ordinary channel operation.
fn admit(context: &JobContext) -> Result<(), NativeObservationError> {
    if context.cancellation_requested()
        || context.deadline().is_none_or(|until| context.now() >= until)
    {
        return Err(invalid("activation_channel_admission"));
    }
    Ok(())
}
use operation::{Direction, Operation};
pub use pair::{InheritedChannel, PreparedChannel, capture_inherited};

/// Covers both endpoints, their bounded frames, native operation storage and returned frame loans.
pub const CHANNEL_RESERVATION_BYTES: u64 = (4 * MAX_FRAME_BYTES + 65536) as u64;

/// A complete bounded frame retains its original admission through the last consumer.
#[derive(Clone)]
pub struct ChannelFrame {
    bytes: Arc<Vec<u8>>,
    _memory: Arc<MemoryClaim>,
}
impl ChannelFrame {
    /// Borrow immutable bytes; authentication and application meaning belong to the launch broker.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// Observation survives lost worker results and can be polled again without repeating I/O.
pub enum ChannelProgress {
    /// Windows still owns the operation's stable buffers.
    Pending,
    /// The entire outgoing frame completed; this is not peer receipt or application creation.
    Sent,
    /// Exactly one incoming frame completed and remains retained by the endpoint.
    Received(ChannelFrame),
}

/// Retain this owner outside worker results. All methods that perform I/O run on admitted workers.
/// Each endpoint sends and receives at most one frame, sequentially. The host schedules polls
/// through its execution owner and keeps its original cutoff. No method blocks for peer activity.
#[derive(Clone)]
pub struct ChannelEndpoint(Arc<Mutex<State>>);
/// Keep physical outputs, operation results and both original execution scopes outside worker results.
struct State {
    pipe: Option<Handle>,
    operation: Option<Box<Operation>>,
    memory: Arc<MemoryClaim>,
    original: Option<JobContext>,
    cleanup: Option<JobContext>,
    sent: bool,
    received: Option<ChannelFrame>,
    last: Option<Direction>,
    closing: bool,
}
impl Drop for State {
    fn drop(&mut self) {
        if self.operation.as_ref().is_some_and(|operation| operation.pending()) {
            // The kernel still owns pointers into this operation. Dedicated launcher/engine
            // process exit is the last-resort boundary; freeing them here would be use-after-free.
            tracing::error!(
                operation = "activation_channel_unsettled",
                "Pending channel storage retained until process exit"
            );
            std::mem::forget(self.operation.take());
            std::mem::forget(self.pipe.take());
            std::mem::forget(self.memory.clone());
        }
    }
}
impl ChannelEndpoint {
    /// Attach already-owned native custody to the original admitted execution scope.
    fn new(pipe: Handle, memory: Arc<MemoryClaim>, context: &JobContext) -> Self {
        Self(Arc::new(Mutex::new(State {
            pipe: Some(pipe),
            operation: None,
            memory,
            original: Some(context.clone()),
            cleanup: None,
            sent: false,
            received: None,
            last: None,
            closing: false,
        })))
    }
    /// Start one bounded outgoing frame. Keep this endpoint even if admission or I/O fails.
    /// # Errors
    /// Rejects replay, concurrent operations, oversized frames, original cancellation/expiry,
    /// foreign execution ownership, allocation failures and native issue errors.
    pub fn send(&self, bytes: &[u8], context: &JobContext) -> Result<(), NativeObservationError> {
        admit(context)?;
        let mut state = crate::scope::lock(&self.0);
        state.admit(context)?;
        if state.sent || state.last == Some(Direction::Write) || state.operation.is_some() {
            return Err(invalid("activation_channel_send_closed"));
        }
        let operation = Operation::write(bytes, state.memory.clone())?;
        state.last = Some(Direction::Write);
        state.operation = Some(Box::new(operation));
        state.issue()
    }
    /// Start the sole incoming frame, enforcing its length before allocating body storage.
    /// # Errors
    /// Rejects replay, concurrent operations, closed/originally cancelled admission and native errors.
    pub fn receive(&self, context: &JobContext) -> Result<(), NativeObservationError> {
        admit(context)?;
        let mut state = crate::scope::lock(&self.0);
        state.admit(context)?;
        if state.received.is_some()
            || state.last == Some(Direction::Read)
            || state.operation.is_some()
        {
            return Err(invalid("activation_channel_receive_closed"));
        }
        let operation = Operation::read(state.memory.clone())?;
        state.last = Some(Direction::Read);
        state.operation = Some(Box::new(operation));
        state.issue()
    }
    /// Observe once without waiting; completed results remain replayable after discarded jobs.
    /// # Errors
    /// Preserves peer closure, malformed framing, native failures and the original admission cutoff.
    pub fn poll(&self, context: &JobContext) -> Result<ChannelProgress, NativeObservationError> {
        admit(context)?;
        let mut state = crate::scope::lock(&self.0);
        state.admit(context)?;
        if state.operation.is_some() {
            let pipe =
                state.pipe.as_ref().ok_or_else(|| invalid("activation_channel_pipe_absent"))?.raw();
            if !state
                .operation
                .as_mut()
                .ok_or_else(|| invalid("activation_channel_operation_absent"))?
                .advance(pipe)?
            {
                return Ok(ChannelProgress::Pending);
            }
            let operation = state
                .operation
                .take()
                .ok_or_else(|| invalid("activation_channel_operation_absent"))?;
            match state.last {
                Some(Direction::Write) => state.sent = true,
                Some(Direction::Read) => {
                    state.received = Some(ChannelFrame {
                        bytes: Arc::new(operation.into_bytes()),
                        _memory: state.memory.clone(),
                    })
                }
                None => return Err(invalid("activation_channel_direction_absent")),
            }
        }
        match state.last {
            Some(Direction::Write) if state.sent => Ok(ChannelProgress::Sent),
            Some(Direction::Read) => state
                .received
                .clone()
                .map(ChannelProgress::Received)
                .ok_or_else(|| invalid("activation_channel_receive_absent")),
            _ => Err(invalid("activation_channel_not_started")),
        }
    }
    /// Close admission, request cancellation, observe its actual completion and close native outputs.
    /// Use independent cleanup admission with the same execution owner and a fixed cleanup cutoff.
    /// `false` retains all pending storage for the next scheduled observation; it is not settlement.
    /// # Errors
    /// Preserves native cancellation/completion/close errors and still-owned outputs for retry.
    pub fn retire(&self, context: &JobContext) -> Result<bool, NativeObservationError> {
        admit(context)?;
        let mut state = crate::scope::lock(&self.0);
        if state.original.as_ref().is_some_and(|original| original.owner_id() != context.owner_id())
        {
            return Err(invalid("activation_channel_execution_owner"));
        }
        if let Some(original) = &state.cleanup {
            admit(original)?;
            if original.owner_id() != context.owner_id() {
                return Err(invalid("activation_channel_execution_owner"));
            }
        } else {
            state.cleanup = Some(context.clone());
        }
        state.closing = true;
        let pipe = state.pipe.as_ref().map(Handle::raw);
        if let Some(operation) = &mut state.operation
            && !operation.retire(pipe.ok_or_else(|| invalid("activation_channel_pipe_absent"))?)?
        {
            return Ok(false);
        }
        state.operation = None;
        if let Some(pipe) = &mut state.pipe {
            pipe.close()?;
        }
        state.pipe = None;
        state.received = None;
        state.original = None;
        Ok(true)
    }
}
impl State {
    /// Later worker jobs cannot replace the original execution owner, cancellation or cutoff.
    fn admit(&mut self, context: &JobContext) -> Result<(), NativeObservationError> {
        if self.closing || self.pipe.is_none() {
            return Err(invalid("activation_channel_closed"));
        }
        if let Some(original) = &self.original {
            admit(original)?;
            if original.owner_id() != context.owner_id() {
                return Err(invalid("activation_channel_execution_owner"));
            }
        } else {
            self.original = Some(context.clone());
        }
        Ok(())
    }
    /// Issue only after the stable operation has entered externally retained endpoint custody.
    fn issue(&mut self) -> Result<(), NativeObservationError> {
        let pipe =
            self.pipe.as_ref().ok_or_else(|| invalid("activation_channel_pipe_absent"))?.raw();
        self.operation
            .as_mut()
            .ok_or_else(|| invalid("activation_channel_operation_absent"))?
            .issue(pipe)
    }
}
