// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Revalidate current authenticated transport independently of historical peer observations.
use super::*;
use sugarsubstitute_contracts::execution::{AdmissionError, JobPhase};

enum Rejection {
    Admission(AdmissionError),
    Pipe(LifecyclePipeError),
}
impl Rejection {
    fn retain(self) -> NativeObservationError {
        match self {
            Self::Admission(cause) => NativeObservationError::new("pipe_current_admission", cause),
            Self::Pipe(cause) => NativeObservationError::new("pipe_current_admission", cause),
        }
    }
}
impl LifecyclePipeControl {
    /// Check this actual authenticated transport immediately before application effects.
    /// Historical peer verification does not keep a stopped session eligible. The actual
    /// Core clock is sampled outside every owner lock; the final Core->Shared fence checks
    /// retained job identity, running phase, native slots, cancellation and all current
    /// scope ancestors. An authenticated passive session keeps its cleared handshake timer.
    /// This observation grants no handles or authority to defer later effect checks.
    /// # Errors
    /// Retains the actual provider/admission cause or typed closed/not-ready rejection.
    /// Error allocation happens after all Core/Shared guards are released.
    pub fn validate_current_admission(&self) -> Result<(), NativeObservationError> {
        let core = self
            .inner
            .shared
            .core
            .upgrade()
            .ok_or_else(|| Rejection::Pipe(LifecyclePipeError::Closed).retain())?;
        let now = core
            .clock
            .metadata()
            .map_err(|cause| Rejection::Admission(AdmissionError::Unexpected(cause)).retain())?;
        let checked = (|| {
            let state = lock(&core.state);
            let record = state
                .jobs
                .get(&self.inner.shared.id)
                .ok_or(Rejection::Pipe(LifecyclePipeError::Closed))?;
            if !self.inner.owner.matches_record(record)
                || record.phase != JobPhase::Running
                || record.retiring
            {
                return Err(Rejection::Pipe(LifecyclePipeError::Closed));
            }
            let cutoff = crate::admission_liveness::scope_deadline(&core, &state, record)
                .map_err(Rejection::Admission)?;
            if let Some(deadline) = cutoff.filter(|deadline| *deadline <= now) {
                return Err(Rejection::Admission(AdmissionError::DeadlineExpired {
                    deadline,
                    now,
                }));
            }
            let slots =
                record.native_deadlines.ok_or(Rejection::Pipe(LifecyclePipeError::Closed))?;
            if slots.stopped()
                || crate::symbol_admission::original_stop(&state, self.inner.shared.id, now)
                    .is_some()
            {
                return Err(Rejection::Pipe(LifecyclePipeError::Closed));
            }
            if let Some(deadline) = slots.expired_deadline(now) {
                return Err(Rejection::Admission(AdmissionError::DeadlineExpired {
                    deadline,
                    now,
                }));
            }
            let pipe = lock(&self.inner.shared.state);
            if pipe.stopped {
                return Err(Rejection::Pipe(LifecyclePipeError::Closed));
            }
            if !pipe.connected || !pipe.authenticated {
                return Err(Rejection::Pipe(LifecyclePipeError::NotReady));
            }
            Ok(())
        })();
        checked.map_err(Rejection::retain)
    }
}
