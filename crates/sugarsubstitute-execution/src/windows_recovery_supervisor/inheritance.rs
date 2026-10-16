// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Transfer genuine read-only Root bootstrap custody through the recovery carrier inheritance list.
use super::*;
use crate::windows_lifecycle_pipe::LifecyclePipeControl;

/// Two actual same-Root pins, retained until the carrier's attribute list is deleted.
/// Neither fields, numeric handles nor a decoded grant can construct this pair.
pub struct RecoveryCarrierBootstrap {
    recovery: RecoveryBootstrap,
    desktop: LifecyclePipeBootstrap,
}
/// Preserve both genuine pins on a rejected pairing without dropping native ownership.
pub struct RecoveryCarrierBootstrapRejection {
    /// Actual original source/current admission or exact pair mismatch.
    pub error: AdmissionError,
    /// Original recovery pin, unchanged and still paired with its enrollment.
    pub recovery: RecoveryBootstrap,
    /// Original Root UI pin, unchanged and still paired with its enrollment.
    pub desktop: LifecyclePipeBootstrap,
}
impl RecoveryPipeControl {
    /// Pair this genuine Root grant/session with its actual pre-admitted direct UI server.
    /// Both fixed pipes and their original deadlines already exist before the durable ACK.
    /// The carrier inherits genuine OS sections; capture tokens/addresses and coordinator
    /// authority are never serialized. A fresh capture owner separately creates the UI input.
    /// # Errors
    /// Returns both identical pins on foreign/replayed/stopped grant, wrong allocation,
    /// cross-run/purpose, duplicate section, clock or extended original deadline.
    #[expect(
        clippy::result_large_err,
        reason = "Return both original read-only section pins under their existing pipe metadata claims on rejection"
    )]
    pub fn pair_desktop_bootstrap(
        &self,
        grant: &RegisteredRecoveryGrant,
        desktop_control: &LifecyclePipeControl,
        recovery: RecoveryBootstrap,
        desktop: LifecyclePipeBootstrap,
    ) -> Result<RecoveryCarrierBootstrap, RecoveryCarrierBootstrapRejection> {
        let decision = (|| {
            let core =
                self.inner.shared.core.upgrade().ok_or(AdmissionError::InvalidCaptureAdmission)?;
            let now = core.clock.metadata().map_err(AdmissionError::Unexpected)?;
            let state = lock(&core.state);
            grant.state()?.validate_locked(&core, &state, now)?;
            if self.prepared_key() != Some(grant.key())
                || recovery.key != grant.key()
                || !self.inner.shared.same_allocation(&recovery.inner.shared)
                || !desktop_control.inner.shared.same_allocation(&desktop.shared)
                || !desktop.shared.core.ptr_eq(&std::sync::Arc::downgrade(&core))
                || desktop.shared.child
                || desktop.shared.captured
                || desktop.shared.original_deadline > grant.original_budget().until()
                || desktop.shared.original_deadline != self.inner.shared.original_deadline
            {
                return Err(AdmissionError::InvalidCaptureAdmission);
            }
            crate::lifecycle_pipe::current_admission::prepared_bootstrap_locked(
                &core,
                &state,
                &desktop_control.inner,
                &desktop,
                now,
            )?;
            crate::lifecycle_pipe::current_admission::prepared_bootstrap_locked(
                &core,
                &state,
                &self.inner,
                &recovery.inner,
                now,
            )?;
            let root = lock(&self.inner.shared.state);
            let ui = lock(&desktop.shared.state);
            let root_until = root.deadline.ok_or(AdmissionError::InvalidCaptureAdmission)?;
            let ui_until = ui.deadline.ok_or(AdmissionError::InvalidCaptureAdmission)?;
            if root.binding != Some(PeerBinding::Recovery(grant.key()))
                || ui.binding != Some(PeerBinding::Desktop(grant.key().run()))
                || !root.pin
                || !ui.pin
                || root.inherited == 0
                || root.inherited == ui.inherited
                || ui_until.frequency() != root_until.frequency()
                || ui.enrolled
                || ui.peer.is_some()
                || root.enrolled
                || root.peer.is_some()
            {
                return Err(AdmissionError::InvalidCaptureAdmission);
            }
            Ok(())
        })();
        match decision {
            Ok(()) => Ok(RecoveryCarrierBootstrap { recovery, desktop }),
            Err(error) => Err(RecoveryCarrierBootstrapRejection { error, recovery, desktop }),
        }
    }
}
impl RecoveryCarrierBootstrap {
    /// Return exact grant matching facts while both actual pins remain retained.
    pub const fn key(&self) -> RecoveryGrantKey {
        self.recovery.key()
    }
    /// Return the immutable original carrier counter; no translation/renewal occurs.
    pub fn deadline(&self) -> Option<LifecyclePipeDeadline> {
        self.recovery.deadline()
    }
    /// Verify the recovery pin still belongs to its actual suspended carrier enrollment.
    pub fn matches_enrollment(&self, enrollment: &RecoveryEnrollment) -> bool {
        self.recovery.matches_enrollment(enrollment)
    }
    /// Borrow exactly two distinct read-only sections for the native inheritance allowlist.
    pub fn borrow_inheritance(&self) -> [LifecyclePipeInheritance<'_>; 2] {
        [self.recovery.borrow_inheritance(), self.desktop.borrow_inheritance()]
    }
}
/// Lexical dedicated carrier input; native adoption verifies both actual inherited sections.
/// Construction grants no registration, source, native or coordinator authority.
pub struct InheritedRecoveryBootstrap {
    recovery: InheritedLifecycleBootstrap,
    desktop: InheritedLifecycleBootstrap,
}
impl InheritedRecoveryBootstrap {
    /// Validate two distinct nonpseudo inherited slots without taking native ownership.
    /// # Errors
    /// Rejects zero, overflow, pseudo handles or duplicate sections before native admission.
    pub fn new(recovery: u64, desktop: u64) -> Result<Self, LifecyclePipeError> {
        validate_pair_slots(recovery, desktop)?;
        Ok(Self {
            recovery: InheritedLifecycleBootstrap::new(recovery)?,
            desktop: InheritedLifecycleBootstrap::new(desktop)?,
        })
    }
    pub(crate) fn into_raw(self) -> [usize; 2] {
        [self.recovery.into_raw(), self.desktop.into_raw()]
    }
}
impl std::fmt::Debug for RecoveryCarrierBootstrapRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RecoveryCarrierBootstrapRejection { redacted }")
    }
}
impl std::fmt::Display for RecoveryCarrierBootstrapRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Genuine recovery bootstrap pair rejected")
    }
}
impl Error for RecoveryCarrierBootstrapRejection {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.error)
    }
}

/// Reject aliasing and nonowned numeric slots before either native owner adopts a handle.
pub(crate) fn validate_pair_slots(recovery: u64, desktop: u64) -> Result<(), LifecyclePipeError> {
    let first = usize::try_from(recovery).map_err(|_| LifecyclePipeError::InvalidMessage)?;
    let second = usize::try_from(desktop).map_err(|_| LifecyclePipeError::InvalidMessage)?;
    if first == second
        || first == 0
        || second == 0
        || first >= usize::MAX - 15
        || second >= usize::MAX - 15
    {
        Err(LifecyclePipeError::InvalidMessage)
    } else {
        Ok(())
    }
}
