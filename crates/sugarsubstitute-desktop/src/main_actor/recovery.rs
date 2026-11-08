// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bridge fixed recovery presentation to the same Main's retained native caption peer.
use super::*;
use crate::{MainBackendState, main_backend::RetryAction};
use sugarsubstitute_contracts::lifecycle::LeaseKey;
/// Preserve the original reservation; include largest four-peer/retry layouts before native effects.
pub(super) const NATIVE_BYTES: u64 = 512 * 1024;
/// Reject an oversized compiled four-peer owner before the original metadata claim/native initialization.
pub(super) fn check_storage() -> Result<(), DesktopError> {
    let required = Controls::storage_bytes(4)
        .and_then(|controls| controls.checked_add(size_of::<MainActor>()))
        .and_then(|bytes| bytes.checked_add(placement::STORAGE_BYTES as usize));
    if required.is_none_or(|bytes| bytes as u64 > NATIVE_BYTES) {
        return Err(DesktopError::message(
            "claim_main_native_metadata",
            "largest native retry metadata exceeds original reservation",
        ));
    }
    Ok(())
}
impl MainActor {
    /// Apply only the current Root-supplied outage data on this existing actor.
    pub(crate) fn present_backend(
        &mut self,
        identity: crate::MainIdentity,
        failed: LeaseKey,
        state: MainBackendState,
    ) -> Result<(), DesktopError> {
        if self.admission.closing() || self.retired {
            return Err(DesktopError::message("present_main_backend", "Main admission is closed"));
        }
        let change = self.recovery.present(identity, failed, state);
        self.finish_recovery_presentation(change)
    }
    /// Present exact authenticated handoff data while preserving an unresolved outage.
    pub(crate) fn replace_backend(
        &mut self,
        identity: crate::MainIdentity,
        previous: LeaseKey,
        failed: LeaseKey,
        state: MainBackendState,
    ) -> Result<(), DesktopError> {
        if self.admission.closing() || self.retired {
            return Err(DesktopError::message("replace_main_backend", "Main admission is closed"));
        }
        let change = self.recovery.replace(identity, previous, failed, state);
        self.finish_recovery_presentation(change)
    }
    fn finish_recovery_presentation(
        &mut self,
        change: Result<bool, crate::main_backend::RecoveryError>,
    ) -> Result<(), DesktopError> {
        match change {
            Ok(false) => Ok(()),
            Ok(true) => self.present_recovery_control(),
            Err(cause) => {
                let exhausted = cause == crate::main_backend::RecoveryError::RevisionExhausted;
                let mut failure = Some(DesktopError::new("present_main_backend", cause));
                if exhausted && let Err(native) = self.present_recovery_control() {
                    DesktopError::retain_first(&mut failure, native);
                }
                Err(failure.ok_or_else(|| {
                    DesktopError::message(
                        "present_main_backend",
                        "original recovery failure missing",
                    )
                })?)
            }
        }
    }
    /// Consume a current native tuple and disarm the peer before returning host input.
    pub(crate) fn retry_backend(
        &mut self,
        action: RetryAction,
    ) -> Result<Option<DesktopStartupObservation>, DesktopError> {
        if self.admission.closing() || self.retired {
            return Ok(None);
        }
        let mut failure = None;
        let observation = match self.recovery.consume(action) {
            Ok(observation) => observation,
            Err(cause) => {
                DesktopError::retain_first(
                    &mut failure,
                    DesktopError::new("native_backend_retry", cause),
                );
                None
            }
        };
        // Consume/exhaustion changes pure state first. Disable the old native
        // mapping/control before any host observation, preserving first errors.
        if let Err(native) = self.present_recovery_control() {
            DesktopError::retain_first(&mut failure, native);
        }
        match failure {
            Some(cause) => Err(cause),
            None => Ok(observation),
        }
    }
    /// Reuse the retained fourth peer without reentering browser or navigation owners.
    pub(super) fn present_recovery_control(&mut self) -> Result<(), DesktopError> {
        if self.controls.is_none() {
            return Ok(());
        }
        let presentation = self.recovery.presentation(self.request()?.settings().recovery);
        if let Some(controls) = &mut self.controls {
            controls.present_retry(presentation)?;
        }
        Ok(())
    }
}
