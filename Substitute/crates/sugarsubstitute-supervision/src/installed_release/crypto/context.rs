// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind CNG effects to the identical original execution owner.
use super::*;
impl CryptoOwner {
    pub(super) fn bind(&mut self, current: &JobContext) -> Result<(), ClaimError> {
        if let Some(CaptureCryptoOrigin { original, causes }) =
            self.origin.as_mut().and_then(|origin| origin.capture.as_mut())
        {
            let context = match original.original().original_context() {
                Ok(context) => context.clone(),
                Err(cause) => return Err(causes[self.active].selected_control(cause, false)),
            };
            self.original = Some(context);
            return self.check_main(current);
        }
        #[cfg(test)]
        {
            let Some(SymbolCryptoOrigin { handles, .. }) =
                self.origin.as_ref().and_then(|origin| origin.symbol.as_ref())
            else {
                return Err(ClaimError::capture_allocation(
                    crate::installed_release::allocation::CaptureCapacityFailure::missing(),
                ));
            };
            let lease = current
                .native_handles()
                .ok_or_else(|| ClaimError::invalid("release_crypto_unadmitted"))?;
            if !handles.matches_lease(lease) {
                return Err(ClaimError::invalid("release_crypto_foreign_ledger"));
            }
            if self.original.is_none() {
                self.original = Some(current.clone());
            }
            self.check_main(current)
        }
        #[cfg(not(test))]
        {
            let _ = current;
            Err(ClaimError::capture_allocation(
                crate::installed_release::allocation::CaptureCapacityFailure::missing(),
            ))
        }
    }
    pub(super) fn check_main(&mut self, current: &JobContext) -> Result<(), ClaimError> {
        if let Some(CaptureCryptoOrigin { original, causes }) =
            self.origin.as_mut().and_then(|origin| origin.capture.as_mut())
        {
            let observed = original.original().check_effect();
            causes[self.active].matches_selected(original)?;
            return match observed {
                Ok(()) => causes[self.active].check(false),
                Err(cause) => Err(causes[self.active].selected_control(cause, false)),
            };
        }
        #[cfg(test)]
        {
            let Some(SymbolCryptoOrigin { handles, .. }) =
                self.origin.as_ref().and_then(|origin| origin.symbol.as_ref())
            else {
                return Err(ClaimError::capture_allocation(
                    crate::installed_release::allocation::CaptureCapacityFailure::missing(),
                ));
            };
            let first = self
                .original
                .as_ref()
                .ok_or_else(|| ClaimError::invalid("release_crypto_unbound"))?;
            if first.owner_id() != current.owner_id()
                || first.scope_id() != current.scope_id()
                || first.job_id() != current.job_id()
                || first.deadline().is_none()
                || current.deadline().is_none()
                || current.deadline() > first.deadline()
                || current.native_handles().is_none_or(|lease| !handles.matches_lease(lease))
            {
                return Err(ClaimError::invalid("release_crypto_changed_context"));
            }
            check_live(first)?;
            check_live(current)
        }
        #[cfg(not(test))]
        {
            let _ = current;
            Err(ClaimError::capture_allocation(
                crate::installed_release::allocation::CaptureCapacityFailure::missing(),
            ))
        }
    }
}
#[cfg(test)]
pub(super) fn check_live(context: &JobContext) -> Result<(), ClaimError> {
    if context.cancellation_requested() {
        return Err(ClaimError::invalid("release_crypto_cancelled"));
    }
    let deadline =
        context.deadline().ok_or_else(|| ClaimError::invalid("release_crypto_unbounded"))?;
    if context.now() >= deadline {
        return Err(ClaimError::invalid("release_crypto_expired"));
    }
    Ok(())
}
