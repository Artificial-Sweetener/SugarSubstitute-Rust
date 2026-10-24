// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain original transport admission through ports and consumer-held frames.
use std::{
    error::Error,
    fmt, io,
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};
use sugarsubstitute_contracts::execution::{MemoryClaim, MemoryLease};

/// Shared original reservation; the last owner deallocates controls before releasing credit.
pub(super) struct Budget(Option<Arc<State>>);
/// Admission and delivery controls shared only by this concrete transport pair.
struct State {
    closed: AtomicBool,
    frames: AtomicUsize,
    claim: OnceLock<MemoryClaim>,
}

impl Budget {
    /// Construct inert controls; buffer admission occurs in the actual transport job.
    pub(super) fn new() -> Self {
        Self(Some(Arc::new(State {
            closed: AtomicBool::new(false),
            frames: AtomicUsize::new(0),
            claim: OnceLock::new(),
        })))
    }

    /// Claim the existing job reservation before binding its listener or accepting bytes.
    pub(super) fn activate(&self, memory: &MemoryLease) -> io::Result<()> {
        let claim = memory.claim(super::RESERVATION_BYTES).map_err(io::Error::other)?;
        self.state()?.claim.set(claim).map_err(|_| io::Error::other("Transport already admitted"))
    }

    /// Observe whether this pair has received its original admission.
    pub(super) fn admitted(&self) -> bool {
        self.0.as_ref().is_some_and(|state| state.claim.get().is_some())
    }

    /// Stop new delivery when the host endpoint is dropped, independently of held frames.
    pub(super) fn close(&self) {
        if let Some(state) = &self.0 {
            state.closed.store(true, Ordering::Release);
        }
    }

    /// Observe endpoint retirement without treating it as native process retirement.
    pub(super) fn closed(&self) -> bool {
        self.0.as_ref().is_none_or(|state| state.closed.load(Ordering::Acquire))
    }

    /// Reserve one slot across queued and consumer-held frames, or reject overload.
    pub(super) fn permit(&self) -> io::Result<Permit> {
        if self.closed() {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "Installer closed"));
        }
        if !self.admitted() {
            return Err(io::Error::new(io::ErrorKind::WouldBlock, "Transport not admitted"));
        }
        self.state()?
            .frames
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                (count < super::QUEUE_SLOTS).then_some(count + 1)
            })
            .map_err(|_| io::Error::new(io::ErrorKind::WouldBlock, "Worker frames retained"))?;
        Ok(Permit(self.clone()))
    }

    /// Keep rejected-frame allocations and original causes charged through escaped errors.
    pub(super) fn retain_failure(self, cause: io::Error) -> io::Error {
        io::Error::new(cause.kind(), Failure { cause, _budget: self })
    }

    /// Borrow only the private control block; no reservation capability escapes.
    fn state(&self) -> io::Result<&State> {
        self.0.as_deref().ok_or_else(|| io::Error::other("Transport budget retired"))
    }
}

/// Drop any input allocation retained by an error before releasing its original reservation.
struct Failure {
    cause: io::Error,
    _budget: Budget,
}
impl fmt::Debug for Failure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TransportFailure")
            .field("cause", &self.cause)
            .finish_non_exhaustive()
    }
}
impl fmt::Display for Failure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.cause, formatter)
    }
}
impl Error for Failure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.cause)
    }
}

impl Clone for Budget {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl Drop for Budget {
    fn drop(&mut self) {
        if let Some(state) = self.0.take() {
            drop(Arc::into_inner(state));
        }
    }
}

/// Returned only after its frame allocation drops, so taking events cannot bypass capacity.
pub(super) struct Permit(Budget);
impl Drop for Permit {
    fn drop(&mut self) {
        if let Some(state) = &self.0.0 {
            state.frames.fetch_sub(1, Ordering::AcqRel);
        }
    }
}
