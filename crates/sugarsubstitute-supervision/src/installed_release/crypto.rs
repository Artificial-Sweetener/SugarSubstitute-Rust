// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Authenticate exact bounded ingress while retaining partial CNG outputs and their credit.
mod context;
mod retirement;
use super::{anchor, manifest::MAX_MANIFEST_BYTES};
use crate::ClaimError;
use sha2::{Digest, Sha256};
#[cfg(test)]
use std::sync::Arc;
use std::{error::Error, fmt};
use sugarsubstitute_contracts::execution::JobContext;
#[cfg(test)]
use sugarsubstitute_contracts::execution::{
    JobClass, MemoryClaim, NativeHandleClaim, NativeHandleOutput, SymbolNativePartition,
};
use windows::Win32::Security::Cryptography::*;

/// Preserve the actual CNG NTSTATUS without a stale thread-error lookup or HRESULT conversion.
#[derive(Debug)]
struct Status(i32);
impl fmt::Display for Status {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(output, "CNG status 0x{:08x}", self.0 as u32)
    }
}
impl Error for Status {}

enum CryptoCredit {
    #[cfg(test)]
    Symbol {
        _output: NativeHandleOutput,
    },
    Capture {
        _output: super::outputs::CaptureOutputGuard,
    },
}
#[cfg(test)]
struct SymbolCryptoOrigin {
    _memory: Arc<MemoryClaim>,
    handles: Arc<NativeHandleClaim>,
}
struct CaptureCryptoOrigin {
    original: super::allocation::AllocationOrigin,
    causes: [crate::durable_storage::protected::native_file::NativeCauseDestinations; 2],
}
struct CryptoOrigin {
    #[cfg(test)]
    symbol: Option<SymbolCryptoOrigin>,
    capture: Option<CaptureCryptoOrigin>,
}
/// Keep both pre-reserved CNG cells outside cancellable signature results.
pub(super) struct CryptoOwner {
    provider: usize,
    key: usize,
    provider_credit: Option<CryptoCredit>,
    key_credit: Option<CryptoCredit>,
    origin: Option<CryptoOrigin>,
    original: Option<JobContext>,
    attempted: bool,
    active: usize,
    first_failure: Option<ClaimError>,
    secondary_failure: Option<ClaimError>,
}
impl CryptoOwner {
    /// Reserve both possible native outputs before any CNG effect.
    /// # Errors
    /// Rejects a different partition or exhausted actual original output share.
    #[cfg(test)]
    pub(super) fn new(
        memory: Arc<MemoryClaim>,
        handles: Arc<NativeHandleClaim>,
    ) -> Result<Self, ClaimError> {
        if handles.partition() != SymbolNativePartition::SelectedRelease {
            return Err(ClaimError::invalid("release_crypto_partition"));
        }
        let provider_credit = handles
            .reserve_output()
            .map_err(|cause| ClaimError::new("release_crypto_capacity", cause))?;
        let key_credit = handles
            .reserve_output()
            .map_err(|cause| ClaimError::new("release_crypto_capacity", cause))?;
        Ok(Self {
            provider: 0,
            key: 0,
            provider_credit: Some(CryptoCredit::Symbol { _output: provider_credit }),
            key_credit: Some(CryptoCredit::Symbol { _output: key_credit }),
            origin: Some(CryptoOrigin {
                symbol: Some(SymbolCryptoOrigin { _memory: memory, handles }),
                capture: None,
            }),
            original: None,
            attempted: false,
            active: 0,
            first_failure: None,
            secondary_failure: None,
        })
    }

    /// Retain both genuine selected-release credits with the original reconstructed owner.
    pub(super) fn selected(
        original: super::allocation::AllocationOrigin,
        provider: super::outputs::CaptureOutputGuard,
        key: super::outputs::CaptureOutputGuard,
        causes: [crate::durable_storage::protected::native_file::NativeCauseDestinations; 2],
    ) -> Result<Self, ClaimError> {
        if !provider.original().matches(&original) || !key.original().matches(&original) {
            return Err(ClaimError::capture_allocation(
                super::allocation::CaptureCapacityFailure::identity(),
            ));
        }
        for causes in &causes {
            causes.matches_selected(&original)?;
        }
        Ok(Self {
            provider: 0,
            key: 0,
            provider_credit: Some(CryptoCredit::Capture { _output: provider }),
            key_credit: Some(CryptoCredit::Capture { _output: key }),
            origin: Some(CryptoOrigin {
                #[cfg(test)]
                symbol: None,
                capture: Some(CaptureCryptoOrigin { original, causes }),
            }),
            original: None,
            attempted: false,
            active: 0,
            first_failure: None,
            secondary_failure: None,
        })
    }
    fn error_invalid(&mut self, operation: &'static str) -> ClaimError {
        match self.origin.as_mut().and_then(|origin| origin.capture.as_mut()) {
            Some(CaptureCryptoOrigin { causes, .. }) => causes[self.active].error(
                operation,
                crate::claim_error::BoundedNativeCause::RawIo(std::io::Error::from(
                    std::io::ErrorKind::InvalidData,
                )),
                false,
            ),
            _ => ClaimError::invalid(operation),
        }
    }
    fn status(
        &mut self,
        operation: &'static str,
        status: i32,
        cleanup: bool,
    ) -> Result<(), ClaimError> {
        if status == 0 {
            return Ok(());
        }
        match self.origin.as_mut().and_then(|origin| origin.capture.as_mut()) {
            Some(CaptureCryptoOrigin { causes, .. }) => Err(causes[self.active].selected_error(
                operation,
                super::causes::CaptureBoundedCause::Status(super::causes::CaptureCryptoStatus(
                    status,
                )),
                cleanup,
            )),
            _ => check_status(operation, status),
        }
    }
    /// Verify the original bytes once under the compiled reviewed anchor.
    /// # Errors
    /// Missing trust, invalid bounds/context, unknown native status and invalid signatures
    /// retain partial outputs and their original memory/credit for independent retirement.
    pub(super) fn verify(
        &mut self,
        bytes: &[u8],
        signature: &[u8],
        context: &JobContext,
    ) -> Result<(), ClaimError> {
        if self.attempted {
            return Err(self.error_invalid("release_crypto_already_attempted"));
        }
        self.attempted = true;
        let result = self.verify_once(bytes, signature, context);
        let result = match self.origin.as_mut().and_then(|origin| origin.capture.as_mut()) {
            Some(CaptureCryptoOrigin { original, causes }) => {
                causes[self.active].selected_post(result, original)
            }
            _ => result,
        };
        if let Err(cause) = &result {
            self.remember(cause.clone());
        }
        result
    }
    fn verify_once(
        &mut self,
        bytes: &[u8],
        signature: &[u8],
        context: &JobContext,
    ) -> Result<(), ClaimError> {
        if bytes.is_empty() || bytes.len() > MAX_MANIFEST_BYTES || signature.len() != 384 {
            return Err(self.error_invalid("release_signature_bound"));
        }
        self.bind(context)?;
        let anchor =
            anchor::selected().ok_or_else(|| self.error_invalid("missing_trusted_release"))?;
        let mut output = BCRYPT_ALG_HANDLE::default();
        self.check_main(context)?;
        // SAFETY: The pre-reserved durable owner exists before the output. The pinned
        // SDK names the fixed RSA primitive provider; no caller/provider lookup is used.
        let status = unsafe {
            BCryptOpenAlgorithmProvider(
                &mut output,
                BCRYPT_RSA_ALGORITHM,
                MS_PRIMITIVE_PROVIDER,
                BCRYPT_OPEN_ALGORITHM_PROVIDER_FLAGS::default(),
            )
        };
        self.provider = output.0 as usize;
        if self.provider == 0 {
            self.provider_credit = None;
        }
        self.status("release_crypto_provider", status.0, false)?;
        if self.provider == 0 {
            return Err(self.error_invalid("release_crypto_missing_provider"));
        }
        self.active = 1;
        self.check_main(context)?;
        let mut output = BCRYPT_KEY_HANDLE::default();
        // SAFETY: The exact reviewed411-byte public blob and retained provider stay
        // live; adoption into the reserved key cell precedes every status inspection.
        let status = unsafe {
            BCryptImportKeyPair(
                self.provider(),
                None,
                BCRYPT_RSAPUBLIC_BLOB,
                &mut output,
                anchor.public_blob,
                0,
            )
        };
        self.key = output.0 as usize;
        if self.key == 0 {
            self.key_credit = None;
        }
        self.status("release_crypto_import", status.0, false)?;
        if self.key == 0 {
            return Err(self.error_invalid("release_crypto_missing_key"));
        }
        self.check_main(context)?;
        let digest: [u8; 32] = Sha256::digest(bytes).into();
        let padding = BCRYPT_PSS_PADDING_INFO { pszAlgId: BCRYPT_SHA256_ALGORITHM, cbSalt: 32 };
        // SAFETY: Exact SHA256/PSS-salt32 inputs and retained actual key are borrowed
        // synchronously. Only canonical STATUS_SUCCESS accepts the signed whole payload.
        let status = unsafe {
            BCryptVerifySignature(
                self.key(),
                Some((&padding as *const BCRYPT_PSS_PADDING_INFO).cast()),
                &digest,
                signature,
                BCRYPT_PAD_PSS,
            )
        };
        self.status("release_manifest_signature", status.0, false)?;
        self.check_main(context)
    }
    fn provider(&self) -> BCRYPT_ALG_HANDLE {
        BCRYPT_ALG_HANDLE(self.provider as *mut _)
    }
    fn key(&self) -> BCRYPT_KEY_HANDLE {
        BCRYPT_KEY_HANDLE(self.key as *mut _)
    }
    fn remember(&mut self, cause: ClaimError) {
        if self.first_failure.is_none() {
            self.first_failure = Some(cause);
        } else if self.secondary_failure.is_none() {
            self.secondary_failure = Some(cause);
        }
    }

    pub(super) fn has_output(&self) -> bool {
        self.provider != 0 || self.key != 0
    }
}
fn check_status(operation: &'static str, status: i32) -> Result<(), ClaimError> {
    if status == 0 { Ok(()) } else { Err(ClaimError::new(operation, Status(status))) }
}
impl Drop for CryptoOwner {
    fn drop(&mut self) {
        if self.provider != 0 || self.key != 0 {
            let retained = Self {
                provider: std::mem::take(&mut self.provider),
                key: std::mem::take(&mut self.key),
                provider_credit: self.provider_credit.take(),
                key_credit: self.key_credit.take(),
                origin: self.origin.take(),
                original: self.original.take(),
                attempted: self.attempted,
                active: self.active,
                first_failure: self.first_failure.take(),
                secondary_failure: self.secondary_failure.take(),
            };
            std::mem::forget(retained);
            tracing::error!(operation = "release_crypto_abandoned", "CNG output custody retained");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_claims::files::test_support::{TestResult, admitted};
    #[test]
    fn bounded_symbol_fixture_rejection_keeps_original_until_null_retirement() -> TestResult {
        admitted(|context| {
            let available = match context.memory().claim(context.memory().bytes()) {
                Ok(claim) => claim.bytes(),
                Err(cause) => cause.available_bytes,
            };
            let memory = Arc::new(
                context
                    .memory()
                    .claim(2_097_152)
                    .map_err(|cause| ClaimError::new("fixture_crypto_memory", cause))?,
            );
            let handles = context
                .native_handles()
                .ok_or_else(|| ClaimError::invalid("fixture_crypto_lease"))?
                .take_partition(SymbolNativePartition::SelectedRelease)
                .map_err(|cause| ClaimError::new("fixture_crypto_partition", cause))?;
            let mut crypto = CryptoOwner::new(memory, Arc::new(handles))?;
            let failure = crypto
                .verify(&[], &[0; 384], &context)
                .err()
                .ok_or_else(|| ClaimError::invalid("fixture_crypto_bound_accepted"))?;
            assert_eq!(failure.operation(), "release_signature_bound");
            assert!(!crypto.has_output());
            assert!(context.memory().claim(available).is_err());
            crypto.retire(&context)?;
            drop(crypto);
            assert_eq!(
                context
                    .memory()
                    .claim(available)
                    .map_err(|cause| ClaimError::new("fixture_crypto_claim_return", cause))?
                    .bytes(),
                available
            );
            Ok(())
        })
    }
}
