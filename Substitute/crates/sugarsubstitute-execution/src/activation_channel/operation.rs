// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Stable overlapped storage survives issue, partial completion, cancellation and checked retirement.
use super::*;
use windows::Win32::{
    Foundation::{
        ERROR_IO_INCOMPLETE, ERROR_IO_PENDING, ERROR_NOT_FOUND, ERROR_OPERATION_ABORTED, HANDLE,
    },
    Storage::FileSystem::{ReadFile, WriteFile},
    System::{
        IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED},
        Threading::{CreateEventW, ResetEvent},
    },
};

/// The endpoint permits only one active direction and one frame in each direction.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Direction {
    Read,
    Write,
}
/// A terminal operation failure remains observable after discarded worker results.
#[derive(Clone)]
enum Failure {
    Native(&'static str, windows::core::Error),
    Invalid(&'static str),
}
impl Failure {
    /// Reconstruct the same stage and cause while preserving retained native custody.
    fn error(&self) -> NativeObservationError {
        match self {
            Self::Native(stage, cause) => NativeObservationError::new(stage, cause.clone()),
            Self::Invalid(stage) => invalid(stage),
        }
    }
}
/// Own every address referenced by the kernel until checked completion and event retirement.
pub(super) struct Operation {
    direction: Direction,
    header: bool,
    bytes: Vec<u8>,
    completed: usize,
    pending: bool,
    cancelled: bool,
    failure: Option<Failure>,
    overlapped: Box<OVERLAPPED>,
    event: Handle,
}
// SAFETY: the endpoint mutex serializes all access. The boxed operation and buffer never move
// or resize while kernel I/O is pending; checked retirement precedes releasing either allocation.
unsafe impl Send for Operation {}
impl Operation {
    #[cfg(test)]
    /// Inject malformed bytes only at the controlled peer's wire boundary.
    pub(super) fn raw_peer(
        bytes: Vec<u8>,
        memory: Arc<MemoryClaim>,
    ) -> Result<Self, NativeObservationError> {
        Self::new(Direction::Write, bytes, memory)
    }
    #[cfg(test)]
    /// Block until the operation's completion event is signalled, without polling sleeps or a
    /// wall-clock bound, so slow native I/O under machine load cannot change a test's outcome.
    pub(super) fn wait_fixture(&self) -> Result<(), NativeObservationError> {
        use windows::Win32::{
            Foundation::WAIT_OBJECT_0,
            System::Threading::{INFINITE, WaitForSingleObject},
        };
        // SAFETY: the controlled fixture holds this event and operation through the wait.
        if unsafe { WaitForSingleObject(self.event.raw(), INFINITE) } != WAIT_OBJECT_0 {
            return Err(invalid("activation_channel_fixture_wait"));
        }
        Ok(())
    }
    /// Allocate only the fixed prefix until the peer supplies a validated body length.
    pub(super) fn read(memory: Arc<MemoryClaim>) -> Result<Self, NativeObservationError> {
        Self::new(Direction::Read, vec![0; 8], memory)
    }
    /// Copy one bounded caller frame into stable kernel-facing storage.
    pub(super) fn write(
        bytes: &[u8],
        memory: Arc<MemoryClaim>,
    ) -> Result<Self, NativeObservationError> {
        if bytes.is_empty() || bytes.len() > MAX_FRAME_BYTES {
            return Err(invalid("activation_channel_frame_bound"));
        }
        let mut frame = Vec::new();
        frame.try_reserve_exact(8 + bytes.len()).map_err(|cause| {
            NativeObservationError::new("activation_channel_frame_memory", cause)
        })?;
        frame.extend_from_slice(b"SL01");
        frame.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        frame.extend_from_slice(bytes);
        Self::new(Direction::Write, frame, memory)
    }
    /// Allocate an exclusively owned event before any I/O can reference this operation.
    fn new(
        direction: Direction,
        bytes: Vec<u8>,
        memory: Arc<MemoryClaim>,
    ) -> Result<Self, NativeObservationError> {
        // SAFETY: a private manual-reset event is immediately adopted with its original reservation.
        let event = unsafe { CreateEventW(None, true, false, None) }
            .map_err(|cause| NativeObservationError::new("activation_channel_event", cause))?;
        Ok(Self {
            direction,
            header: direction == Direction::Read,
            bytes,
            completed: 0,
            pending: false,
            cancelled: false,
            failure: None,
            overlapped: Box::new(OVERLAPPED::default()),
            event: Handle::new(event, memory),
        })
    }
    /// Whether the kernel can still reference the buffer and OVERLAPPED storage.
    pub(super) fn pending(&self) -> bool {
        self.pending
    }
    /// Freeze a terminal failure so later polling cannot reuse an unsuccessful operation.
    fn fail<T>(&mut self, failure: Failure) -> Result<T, NativeObservationError> {
        let error = failure.error();
        self.failure = Some(failure);
        Err(error)
    }
    /// Publish pending custody before issuing one remaining extent, retaining every native output.
    pub(super) fn issue(&mut self, pipe: HANDLE) -> Result<(), NativeObservationError> {
        if let Some(failure) = &self.failure {
            return Err(failure.error());
        }
        if self.pending || self.cancelled || self.completed >= self.bytes.len() {
            return Err(invalid("activation_channel_issue_closed"));
        }
        // SAFETY: no earlier operation remains pending against this exclusively owned event.
        unsafe { ResetEvent(self.event.raw()) }
            .map_err(|cause| NativeObservationError::new("activation_channel_reset", cause))?;
        *self.overlapped = OVERLAPPED { hEvent: self.event.raw(), ..Default::default() };
        self.pending = true;
        // SAFETY: all buffers and OVERLAPPED storage are adopted before issue and remain stable
        // until GetOverlappedResult proves completion. The pipe was verified as asynchronous.
        let result = unsafe {
            match self.direction {
                Direction::Read => ReadFile(
                    pipe,
                    Some(&mut self.bytes[self.completed..]),
                    None,
                    Some(self.overlapped.as_mut()),
                ),
                Direction::Write => WriteFile(
                    pipe,
                    Some(&self.bytes[self.completed..]),
                    None,
                    Some(self.overlapped.as_mut()),
                ),
            }
        };
        match result {
            Ok(()) => Ok(()),
            Err(cause) if native::code(&cause, ERROR_IO_PENDING) => Ok(()),
            Err(cause) => {
                self.pending = false;
                self.fail(Failure::Native("activation_channel_issue", cause))
            }
        }
    }
    /// Adopt actual completion before inspecting its byte count or reporting its error.
    fn observe(&mut self, pipe: HANDLE) -> Result<bool, NativeObservationError> {
        if !self.pending {
            return Ok(true);
        }
        let mut bytes = 0;
        // SAFETY: this nonblocking observation addresses the exact retained live operation.
        let result =
            unsafe { GetOverlappedResult(pipe, self.overlapped.as_ref(), &mut bytes, false) };
        match result {
            Ok(()) => {
                self.pending = false;
                if bytes == 0 || bytes as usize > self.bytes.len() - self.completed {
                    return self.fail(Failure::Invalid("activation_channel_completion_extent"));
                }
                self.completed += bytes as usize;
                Ok(true)
            }
            Err(cause) if native::code(&cause, ERROR_IO_INCOMPLETE) => Ok(false),
            Err(cause) => {
                self.pending = false;
                self.fail(Failure::Native("activation_channel_completion", cause))
            }
        }
    }
    /// Observe once and issue at most one next extent; never wait for peer activity.
    pub(super) fn advance(&mut self, pipe: HANDLE) -> Result<bool, NativeObservationError> {
        if let Some(failure) = &self.failure {
            return Err(failure.error());
        }
        if !self.observe(pipe)? {
            return Ok(false);
        }
        if self.completed == self.bytes.len() && self.header {
            if &self.bytes[..4] != b"SL01" {
                return self.fail(Failure::Invalid("activation_channel_frame_version"));
            }
            let length =
                u32::from_le_bytes(self.bytes[4..8].try_into().map_err(|cause| {
                    NativeObservationError::new("activation_channel_header", cause)
                })?) as usize;
            if length == 0 || length > MAX_FRAME_BYTES {
                return self.fail(Failure::Invalid("activation_channel_frame_bound"));
            }
            self.bytes.try_reserve_exact(length.saturating_sub(self.bytes.len())).map_err(
                |cause| NativeObservationError::new("activation_channel_frame_memory", cause),
            )?;
            self.bytes.resize(length, 0);
            self.completed = 0;
            self.header = false;
        }
        if self.completed < self.bytes.len() {
            self.issue(pipe)?;
            return Ok(false);
        }
        self.event.close()?;
        Ok(true)
    }
    /// Transfer completed receive storage after the endpoint has observed checked retirement.
    pub(super) fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
    /// Request cancellation once, then retain all storage until actual terminal observation.
    pub(super) fn retire(&mut self, pipe: HANDLE) -> Result<bool, NativeObservationError> {
        if self.pending && !self.cancelled {
            // SAFETY: cancellation targets this exact operation; it does not establish completion.
            if let Err(cause) = unsafe { CancelIoEx(pipe, Some(self.overlapped.as_ref())) }
                && !native::code(&cause, ERROR_NOT_FOUND)
            {
                return Err(NativeObservationError::new("activation_channel_cancel", cause));
            }
            self.cancelled = true;
        }
        if self.pending {
            match self.observe(pipe) {
                Ok(false) => return Ok(false),
                Ok(true) => {}
                Err(cause) => {
                    if !matches!(&self.failure, Some(Failure::Native(_, error)) if native::code(error, ERROR_OPERATION_ABORTED))
                    {
                        return Err(cause);
                    }
                }
            }
        }
        self.event.close()?;
        Ok(true)
    }
}
