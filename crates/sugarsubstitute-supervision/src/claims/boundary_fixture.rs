// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Substitute only native filesystem results while real claims jobs produce observations.

use super::*;
use std::sync::Condvar;

/// Control only a deterministic external filesystem call, never execution or event insertion.
#[derive(Clone)]
pub struct Control(Arc<Boundary>);
impl Control {
    /// Pause the next real resolution inside its external native boundary.
    pub fn pause_next(&self) {
        let mut state = lock(&self.0.state);
        state.0 = true;
        state.1 = false;
        state.2 = false;
    }
    /// Block until the paused boundary is actually entered; entry has no wall-clock bound.
    /// # Errors
    /// Reports poisoned synchronization.
    pub fn wait_entered(&self) -> Result<(), ClaimError> {
        let state = lock(&self.0.state);
        drop(
            self.0
                .changed
                .wait_while(state, |state| !state.1)
                .map_err(|_| ClaimError::invalid("claim_fixture_poison"))?,
        );
        Ok(())
    }
    /// Release the same paused call before joining the real executor on every teardown path.
    pub fn release(&self) {
        lock(&self.0.state).2 = true;
        self.0.changed.notify_all();
    }
    /// Return the existing typed original native failure on the next resolution.
    pub fn fail_next(&self) {
        self.0.fail.store(true, Ordering::Release);
    }
    /// Compare the exact retained original failure without exposing a raw cause control.
    pub fn matches_failure(&self, cause: &ClaimError) -> bool {
        self.0.cause.same_primary(cause)
    }
}
/// Retain the supplied inert key and deterministic native output behavior only.
struct Boundary {
    key: CoordinatorKey,
    state: Mutex<(bool, bool, bool)>,
    changed: Condvar,
    fail: AtomicBool,
    cause: ClaimError,
}
impl native_claims::NativePort for Boundary {
    fn initialize(
        &self,
        root: &mut Root,
        _: &std::path::Path,
        _: &sugarsubstitute_contracts::execution::JobContext,
    ) -> Result<(), ClaimError> {
        root.key = Some(self.key);
        root.sid = vec![1];
        root.claimed = true;
        Ok(())
    }
    fn resolve(&self, resolution: &mut Resolution, sid: &[u8]) -> Result<(), ClaimError> {
        let mut state = lock(&self.state);
        if state.0 {
            state.1 = true;
            self.changed.notify_all();
            state = self
                .changed
                .wait_while(state, |state| !state.2)
                .map_err(|_| ClaimError::invalid("claim_fixture_poison"))?;
            state.0 = false;
        }
        drop(state);
        if self.fail.swap(false, Ordering::AcqRel) {
            return Err(self.cause.clone());
        }
        resolution.ready = true;
        resolution.account_sid = sid.to_vec();
        resolution.digest = [2; 32];
        Ok(())
    }
    fn acquire(
        &self,
        backend: &mut Backend,
        _: &std::path::Path,
        _: &sugarsubstitute_contracts::execution::JobContext,
    ) -> Result<(), ClaimError> {
        backend.claimed = true;
        Ok(())
    }
    fn publish(
        &self,
        _: &mut Backend,
        _: native_claims::RecordState,
        _: u64,
        _: &sugarsubstitute_contracts::execution::JobContext,
    ) -> Result<(), ClaimError> {
        Ok(())
    }
    fn release(
        &self,
        backend: &mut Backend,
        _: &sugarsubstitute_contracts::execution::JobContext,
    ) -> Result<(), ClaimError> {
        backend.proof.close()?;
        backend.released = true;
        Ok(())
    }
    fn close(
        &self,
        root: &mut Root,
        _: &sugarsubstitute_contracts::execution::JobContext,
    ) -> Result<(), ClaimError> {
        root.pool = None;
        Ok(())
    }
}
/// Construct real inert state with only native filesystem results replaced.
/// Consumers must initialize and resolve through the original public job admission.
/// This fixture performs no filesystem operations or native authority authentication.
/// # Errors
/// Preserves actual bounded claims-table allocation failure.
pub fn owner(key: CoordinatorKey) -> Result<(WindowsResourceClaims, Control), ClaimError> {
    let mut owner = WindowsResourceClaims::new()?;
    let boundary = Arc::new(Boundary {
        key,
        state: Mutex::new((false, false, false)),
        changed: Condvar::new(),
        fail: AtomicBool::new(false),
        cause: ClaimError::new(
            "claim_fixture_native",
            std::io::Error::other("original boundary cause"),
        ),
    });
    owner.port = boundary.clone();
    Ok((owner, Control(boundary)))
}
