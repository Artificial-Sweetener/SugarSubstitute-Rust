// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Adopt checked CNG close results under the original independent cleanup authority.
#[cfg(test)]
use super::context::check_live;
use super::*;
impl CryptoOwner {
    /// Retire the same key/provider using the independently admitted cleanup context.
    /// # Errors
    /// A changed owner, expired context or actual close status retains remaining custody.
    #[cfg(test)]
    pub(in crate::installed_release) fn retire(
        &mut self,
        context: &JobContext,
    ) -> Result<(), ClaimError> {
        let result = self.retire_once(context);
        if let Err(cause) = &result {
            self.remember(cause.clone());
        }
        result
    }
    #[cfg(test)]
    fn retire_once(&mut self, context: &JobContext) -> Result<(), ClaimError> {
        if self.origin.as_ref().is_some_and(|origin| origin.capture.is_some()) {
            return Err(ClaimError::capture_allocation(
                crate::installed_release::allocation::CaptureCapacityFailure::identity(),
            ));
        }
        if self.provider == 0 && self.key == 0 {
            // An uninvoked/null-producing owner has no native effect to authorize.
            // Its ordinary accounting guards can retire independently of cancellation.
            self.key_credit = None;
            self.provider_credit = None;
            return Ok(());
        }
        let first =
            self.original.as_ref().ok_or_else(|| ClaimError::invalid("release_crypto_unbound"))?;
        if first.owner_id() != context.owner_id()
            || context.spec().class != JobClass::InfrastructureLifecycle
        {
            return Err(ClaimError::invalid("release_crypto_cleanup_context"));
        }
        check_live(context)?;
        if self.key != 0 {
            // SAFETY: This is the same actual imported key. Failure leaves raw and credit armed.
            let status = unsafe { BCryptDestroyKey(self.key()) };
            check_status("release_crypto_destroy", status.0)?;
            self.key = 0;
            self.key_credit = None;
            check_live(context)?;
        }
        if self.provider != 0 {
            // SAFETY: The retained provider is retired only after actual checked key destruction.
            let status = unsafe { BCryptCloseAlgorithmProvider(self.provider(), 0) };
            check_status("release_crypto_close", status.0)?;
            self.provider = 0;
            self.provider_credit = None;
            check_live(context)?;
        }
        self.key_credit = None;
        self.provider_credit = None;
        Ok(())
    }
    /// Check each fresh capture close independently; late returned success adopts
    /// exactly the already accepted output without renewed proof/provider sampling.
    pub(in crate::installed_release) fn retire_capture(
        &mut self,
        proof: &sugarsubstitute_execution::CaptureBundleRetirement,
    ) -> Result<(), ClaimError> {
        for key in [true, false] {
            self.active = usize::from(key);
            let raw = if key { self.key } else { self.provider };
            if raw == 0 {
                if key {
                    self.key_credit = None;
                } else {
                    self.provider_credit = None;
                }
                continue;
            }
            let Some(CaptureCryptoOrigin { original, causes }) =
                self.origin.as_mut().and_then(|origin| origin.capture.as_mut())
            else {
                return Err(ClaimError::capture_allocation(
                    crate::installed_release::allocation::CaptureCapacityFailure::identity(),
                ));
            };
            if !proof.matches_origin(original.original()) {
                return Err(ClaimError::capture_allocation(
                    crate::installed_release::allocation::CaptureCapacityFailure::identity(),
                ));
            }
            causes[self.active].check(true)?;
            if let Err(cause) = proof.check() {
                return Err(causes[self.active].selected_control(cause, true));
            }
            // SAFETY: This identical imported key/provider remains owned throughout
            // its accepted finite call; failed status leaves raw and credit retained.
            let status = unsafe {
                if key {
                    BCryptDestroyKey(self.key())
                } else {
                    BCryptCloseAlgorithmProvider(self.provider(), 0)
                }
            };
            self.status(
                if key { "release_crypto_destroy" } else { "release_crypto_close" },
                status.0,
                true,
            )?;
            if key {
                self.key = 0;
                self.key_credit = None;
            } else {
                self.provider = 0;
                self.provider_credit = None;
            }
        }
        Ok(())
    }
}
