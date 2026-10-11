// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain launch pins and one accounted delivery loan independently of the view.
use super::*;
use crate::task::ErasedJob;
/// Own continuous output drainage and its actual native terminal observation.
pub struct WindowsOutputSession {
    /// Fixed output/launch operations; keep with the shared resource lifetime.
    pub control: WindowsOutputControl,
    /// Terminal occurs only after actual I/O, callbacks and handles retire.
    pub completion: OwnedJob<WindowsOutputReport, NativeObservationError>,
}
/// Observe bounded captured bytes without granting process shutdown authority.
pub struct WindowsOutputControl {
    pub(crate) shared: Arc<Shared>,
    pub(crate) job: Arc<Job<WindowsOutputReport, NativeObservationError>>,
}
impl WindowsOutputControl {
    /// Return current retained facts even after native terminal; no text is decoded.
    pub fn snapshot(&self) -> WindowsOutputReport {
        lock(&self.shared.state).report(self.shared.resource)
    }
    /// Take the sole exact three-handle launch artifact after checked preparation.
    pub fn take_child_stdio(&self) -> Result<WindowsChildStdio, WindowsOutputError> {
        let mut s = lock(&self.shared.state);
        if s.stopped {
            return Err(WindowsOutputError::Closed);
        }
        if !s.prepared {
            return Err(WindowsOutputError::NotReady);
        }
        if s.stdio_taken {
            return Err(WindowsOutputError::AlreadyTransferred);
        }
        s.stdio_taken = true;
        s.pin = true;
        Ok(WindowsChildStdio { shared: self.shared.clone() })
    }
    /// Drain one copied, independently charged authoritative batch.
    /// LoanBusy explicitly rejects a second live guard. EOF is ordered after all
    /// retained same-stream bytes; checked cancellation retirement emits gap-only
    /// facts without EOF. Already-completed native reads may remain passive bytes
    /// or actual EOF after stop without changing the original terminal outcome.
    pub fn take_batch(&self) -> Result<Option<AdmittedOutputBatch>, WindowsOutputError> {
        let mut s = lock(&self.shared.state);
        if s.loan {
            return Err(WindowsOutputError::LoanBusy);
        }
        if !s.prepared && s.memory.is_none() {
            return Err(WindowsOutputError::NotReady);
        }
        let Some((batch, claim)) = s.batch(self.shared.resource)? else { return Ok(None) };
        s.loan = true;
        Ok(Some(AdmittedOutputBatch {
            batch: Some(batch),
            claim: Some(claim),
            shared: self.shared.clone(),
        }))
    }
    /// Request stop; actual EOF and native retirement remain independently observed.
    pub fn close(&self) {
        {
            lock(&self.shared.state).stopped = true;
        }
        self.job.cancel();
        self.shared.changed();
    }
    /// Verify a launch artifact belongs to this exact admitted session and resource.
    pub fn matches_child_stdio(&self, stdio: &WindowsChildStdio) -> bool {
        Arc::ptr_eq(&self.shared, &stdio.shared)
    }
    /// Match a retained close capability to this exact session, not merely its key.
    pub fn matches_cancellation(&self, cancellation: &WindowsOutputCancellation) -> bool {
        Arc::ptr_eq(&self.shared, &cancellation.shared)
    }
}
impl Drop for WindowsOutputControl {
    fn drop(&mut self) {
        self.close();
    }
}
/// Keep a copied OutputBatch charged while its consumer parses the borrowed bytes.
/// No extraction or clone can separate storage from admission. Drop returns the
/// sole delivery slot; held loans never stop the native drain of either stream.
pub struct AdmittedOutputBatch {
    batch: Option<OutputBatch>,
    claim: Option<MemoryClaim>,
    shared: Arc<Shared>,
}
impl AdmittedOutputBatch {
    /// Borrow the bounded authoritative source batch for transcript ingestion.
    pub fn batch(&self) -> Option<&OutputBatch> {
        self.batch.as_ref()
    }
}
impl Drop for AdmittedOutputBatch {
    fn drop(&mut self) {
        drop(self.batch.take());
        drop(self.claim.take());
        {
            lock(&self.shared.state).loan = false;
        }
        self.shared.changed();
    }
}
/// Own exactly stdin/stdout/stderr inherited parent copies from one output session.
/// Keep this pin through CreateProcess and actual attribute deletion. Drop only
/// releases the pin/wakes checked worker close; it never cancels successful output.
/// Failed launches explicitly close their output control and observe retirement.
pub struct WindowsChildStdio {
    pub(crate) shared: Arc<Shared>,
}
impl WindowsChildStdio {
    /// Exact resource generation matching fact, never authentication evidence.
    pub fn resource(&self) -> ResourceKey {
        self.shared.resource
    }
    /// Request output retirement after a failed owned launch. No handle closes here;
    /// this pin must still remain until actual native attribute deletion returns.
    pub fn request_close(&self) {
        request_close(&self.shared);
    }
    /// Transfer from inherited-handle pin to a retained non-pin close capability.
    /// Call only after actual attribute deletion, or when initialization never
    /// committed. Consuming the pin prevents release while its handles are borrowed.
    /// Successful release does not cancel drainage or establish EOF/retirement.
    pub fn release_after_attributes(self) -> WindowsOutputCancellation {
        let cancellation = WindowsOutputCancellation { shared: self.shared.clone() };
        drop(self);
        cancellation
    }
    pub(crate) fn handles(&self, expected: ResourceKey) -> Result<[usize; 3], WindowsOutputError> {
        if expected != self.shared.resource {
            return Err(WindowsOutputError::ForeignResource);
        }
        let s = lock(&self.shared.state);
        if s.stopped || !s.pin {
            return Err(WindowsOutputError::Closed);
        }
        if !s.prepared || s.handles.contains(&0) {
            return Err(WindowsOutputError::NotReady);
        }
        Ok(s.handles)
    }
}
/// Retain exact output retirement authority after inherited copies can close.
/// This owns no process/handle capability; Drop never cancels successful capture.
pub struct WindowsOutputCancellation {
    shared: Arc<Shared>,
}
impl WindowsOutputCancellation {
    /// Return the exact source generation; identity alone grants no privilege.
    pub fn resource(&self) -> ResourceKey {
        self.shared.resource
    }
    /// Request checked output retirement; terminal and actual EOF remain observed.
    pub fn request_close(&self) {
        request_close(&self.shared);
    }
}
fn request_close(shared: &Shared) {
    lock(&shared.state).stopped = true;
    shared.changed();
}
impl Drop for WindowsChildStdio {
    fn drop(&mut self) {
        {
            lock(&self.shared.state).pin = false;
        }
        self.shared.changed();
    }
}
