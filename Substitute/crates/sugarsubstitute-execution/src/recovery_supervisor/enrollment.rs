// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Mint native resume permission only from the genuine Root's exact received enrollment.
use super::{AuthenticatedRecoveryOrigin, ReceivedLifecycleFrame, RecoveryStartupError};
use crate::process_creation::ProcessCreationMemory;
use sugarsubstitute_contracts::{
    execution::AdmissionError,
    recovery_supervisor::{
        RecoveryProcessIdentity,
        wire::{RecoveryCommand, RecoveryReceiver, decode},
    },
};
/// Non-cloneable process-local proof of the original Root's actual enrollment frame.
/// It retains the exact authenticated source and receiver credit, never a native handle.
pub struct RecoveryDesktopEnrollment {
    source: super::startup::SourceRef,
    desktop: RecoveryProcessIdentity,
}
impl AuthenticatedRecoveryOrigin {
    /// Admit the one exact DesktopEnrolled after the completed startup stream.
    /// # Errors
    /// Rejects foreign guard/source, wrong grant/process, replay/gap, premature or
    /// repeated enrollment, cancellation, expiry and original provider failures.
    pub fn admit_desktop_enrollment(
        &self,
        frame: &ReceivedLifecycleFrame,
        expected: RecoveryProcessIdentity,
    ) -> Result<RecoveryDesktopEnrollment, RecoveryStartupError> {
        use std::sync::atomic::Ordering;
        self.check().map_err(RecoveryStartupError::Admission)?;
        if !self.0.creation_taken.load(Ordering::Acquire)
            || !frame.shared.same_allocation(&self.0.shared)
        {
            return Err(RecoveryStartupError::Binding);
        }
        let decoded = decode(frame.as_bytes(), RecoveryReceiver::Supervisor)
            .map_err(RecoveryStartupError::Wire)?;
        if decoded.key != self.0.key
            || !matches!(decoded.command, RecoveryCommand::DesktopEnrolled(identity) if identity == expected)
            || self.0.enrollment_sequence.load(Ordering::Acquire).checked_add(1)
                != Some(decoded.sequence)
        {
            return Err(RecoveryStartupError::Order);
        }
        self.0
            .enrollment_taken
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| RecoveryStartupError::Order)?;
        Ok(RecoveryDesktopEnrollment { source: self.0.clone(), desktop: expected })
    }
}
impl RecoveryDesktopEnrollment {
    /// Recheck genuine source, exact kernel PID/birth and original creation allocation.
    /// This grants only the already-created suspended Desktop's one resume transition;
    /// the native owner separately checks held process/thread identity and its phase.
    /// # Errors
    /// Preserves source/provider/cancellation/expiry rejection before native resume.
    pub fn check(
        &self,
        origin: &AuthenticatedRecoveryOrigin,
        expected: RecoveryProcessIdentity,
        memory: &ProcessCreationMemory,
    ) -> Result<(), AdmissionError> {
        if !self.source.same_allocation(&origin.0) || self.desktop != expected {
            return Err(AdmissionError::InvalidCaptureAdmission);
        }
        origin.check()?;
        memory.check_recovery_source(&self.source)
    }
}
