// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own selected native outputs before effects and retain the complete uncertain capsule.
mod capacity;
mod files;
mod lease;
mod selection;
use super::{
    allocation::*,
    crypto::CryptoOwner,
    names::CaptureNativeName,
    outputs::{CaptureModuleOutput, CaptureOutputGuard, CaptureOutputQuota},
};
use crate::{
    ClaimError,
    native_claims::files::Handle,
    protected_storage::{
        native_file::{NativeCauseDestinations, NativeFile},
        native_name::NativeSid,
    },
};
pub use lease::{
    CaptureBundleLease, CaptureBundleParts, CaptureBundleRetention, CaptureRegistrationFacts,
    CaptureRetirementFailure,
};
use std::{error::Error, fmt};
use sugarsubstitute_execution::{CaptureBundleAdmission, CaptureBundleOrigin};

/// Return partial native custody after a failed actual checked close.
pub struct CaptureOwnerRetirementFailure {
    /// The identical partially selected physical capsule.
    pub owner: CaptureBundleOwner,
    /// Original native or cleanup failure.
    pub cause: ClaimError,
}
pub(super) struct OwnedHandle {
    handle: Handle,
    causes: NativeCauseDestinations,
    identity: Option<[u8; 24]>,
}
pub(super) struct PreparedFile {
    output: CaptureOutputGuard,
    causes: NativeCauseDestinations,
}
pub(super) struct Fields {
    files: [Option<NativeFile>; 8],
    directories: [Option<OwnedHandle>; 80],
    token: Option<OwnedHandle>,
    prepared: [Option<PreparedFile>; 8],
    causes: [Option<NativeCauseDestinations>; 4],
    quota: CaptureOutputQuota,
    scratch: CaptureScratch,
    crypto: Option<CryptoOwner>,
    modules: [Option<CaptureModuleOutput>; 2],
    current: Option<CaptureNativeName>,
    sid: Option<NativeSid>,
    manifest: Vec<u8>,
    signature: Vec<u8>,
    _ingress: CaptureLoan,
    facts: Option<CaptureRegistrationFacts>,
    attempted: bool,
    failure: Option<ClaimError>,
}
/// Retain construction rejection with the original consuming Core admission.
pub struct CaptureConstructionFailure {
    /// The actual pre-effect admission or layout cause.
    pub cause: ClaimError,
    original: CaptureBundleOrigin,
}
impl fmt::Debug for CaptureConstructionFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CaptureConstructionFailure")
            .field("cause", &self.cause)
            .finish_non_exhaustive()
    }
}
impl fmt::Display for CaptureConstructionFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.cause.fmt(f)
    }
}
impl Error for CaptureConstructionFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.cause)
    }
}
impl CaptureConstructionFailure {
    /// Borrow the exact failed target for independently admitted cleanup.
    pub fn original_origin(&self) -> &CaptureBundleOrigin {
        &self.original
    }
}
/// Retain the selected release's actual partial outputs before its first native call.
pub struct CaptureBundleOwner {
    fields: Option<Box<Fields>>,
    allocation: Option<SharedCaptureLoan>,
    original: AllocationOrigin,
}
impl CaptureBundleOwner {
    /// Consume only the real once-issued admission and inert installed-release request.
    /// # Errors
    /// Rejects compiled layouts before allocation/effects while retaining original custody.
    pub fn new(
        _request: super::CaptureSelectionRequest,
        admission: CaptureBundleAdmission,
    ) -> Result<Self, CaptureConstructionFailure> {
        let origin = admission.into_origin();
        if let Err(cause) = capacity::preflight(&origin) {
            return Err(CaptureConstructionFailure {
                cause: ClaimError::capture_allocation(cause),
                original: origin,
            });
        }
        let original = match AllocationOrigin::new(origin) {
            Ok(original) => original,
            Err(failure) => {
                return Err(CaptureConstructionFailure {
                    cause: ClaimError::capture_allocation(failure.cause),
                    original: failure.original,
                });
            }
        };
        match Self::construct(original.clone()) {
            Ok(owner) => Ok(owner),
            Err(cause) => {
                Err(CaptureConstructionFailure { cause, original: original.original().clone() })
            }
        }
    }
    fn construct(original: AllocationOrigin) -> Result<Self, ClaimError> {
        let outside = SharedCaptureLoan::new(
            original
                .reserve(
                    CaptureRegion::Metadata,
                    size_of::<Fields>() + SharedCaptureLoan::control_bytes(),
                )
                .map_err(ClaimError::capture_allocation)?,
        )
        .map_err(ClaimError::capture_allocation)?;
        let quota = CaptureOutputQuota::new(
            SharedCaptureLoan::new(
                original
                    .reserve(
                        CaptureRegion::Metadata,
                        CaptureOutputQuota::control_storage_bytes()
                            + SharedCaptureLoan::control_bytes(),
                    )
                    .map_err(ClaimError::capture_allocation)?,
            )
            .map_err(ClaimError::capture_allocation)?,
        );
        let scratch = original.scratch().map_err(ClaimError::capture_allocation)?;
        let ingress = original
            .reserve(CaptureRegion::Ingress, 65_536 + 384)
            .map_err(ClaimError::capture_allocation)?;
        let mut fields = Box::new(Fields {
            files: std::array::from_fn(|_| None),
            directories: std::array::from_fn(|_| None),
            token: None,
            prepared: std::array::from_fn(|_| None),
            causes: std::array::from_fn(|_| None),
            quota,
            scratch,
            crypto: None,
            modules: [None, None],
            current: None,
            sid: None,
            manifest: Vec::new(),
            signature: Vec::new(),
            _ingress: ingress,
            facts: None,
            attempted: false,
            failure: None,
        });
        let causes = |slot: u8| -> Result<NativeCauseDestinations, ClaimError> {
            Ok(NativeCauseDestinations::selected(
                &original,
                original.cause_destination(slot * 2).map_err(ClaimError::capture_allocation)?,
                original.cause_destination(slot * 2 + 1).map_err(ClaimError::capture_allocation)?,
            ))
        };
        for index in 0..80 {
            let slot = if index < 40 { index } else { index + 9 };
            let handle = Handle::reserve_capture(&mut Some((
                original.clone(),
                fields.quota.reserve_output().map_err(ClaimError::capture_allocation)?,
            )))?;
            fields.directories[index] =
                Some(OwnedHandle { handle, causes: causes(slot as u8)?, identity: None });
        }
        let handle = Handle::reserve_capture(&mut Some((
            original.clone(),
            fields.quota.reserve_output().map_err(ClaimError::capture_allocation)?,
        )))?;
        fields.token = Some(OwnedHandle { handle, causes: causes(48)?, identity: None });
        for index in 0..8 {
            fields.prepared[index] = Some(PreparedFile {
                output: fields.quota.reserve_output().map_err(ClaimError::capture_allocation)?,
                causes: causes((40 + index) as u8)?,
            });
        }
        fields.crypto = Some(CryptoOwner::capture(
            original.clone(),
            fields.quota.reserve_output().map_err(ClaimError::capture_allocation)?,
            fields.quota.reserve_output().map_err(ClaimError::capture_allocation)?,
            [causes(89)?, causes(90)?],
        )?);
        for index in 0..2 {
            fields.modules[index] = Some(CaptureModuleOutput::from_guard(
                fields.quota.reserve_output().map_err(ClaimError::capture_allocation)?,
                causes((91 + index) as u8)?,
            ));
        }
        for index in 0..4 {
            fields.causes[index] = Some(causes((96 + index) as u8)?);
        }
        Ok(Self { fields: Some(fields), allocation: Some(outside), original })
    }
    /// Select once; native and matching failures remain with the same durable owner.
    /// # Errors
    /// Fails closed without a compiled signer or genuine same-file matching association.
    pub fn select(&mut self) -> Result<(), ClaimError> {
        let fields = self.fields.as_mut().ok_or_else(missing)?;
        if fields.attempted {
            return Err(fields.failure.clone().unwrap_or_else(missing));
        }
        fields.attempted = true;
        let result = selection::select(fields, &self.original);
        if let Err(cause) = &result {
            fields.failure = Some(cause.clone());
        }
        result
    }
    /// Move the exact verified native owner into its only consuming lease.
    /// # Errors
    /// A partial or unverified owner never creates a trusted capture lease.
    pub fn take_verified(self) -> Result<CaptureBundleLease, Self> {
        if self
            .fields
            .as_ref()
            .is_some_and(|fields| fields.facts.is_some() && fields.failure.is_none())
        {
            Ok(CaptureBundleLease { owner: self })
        } else {
            Err(self)
        }
    }
    /// Retire a partial or failed selection without creating a trusted lease.
    /// # Errors
    /// Returns the identical complete owner and original native cause on uncertainty.
    pub fn retire(
        self,
        proof: &sugarsubstitute_execution::CaptureBundleRetirement,
    ) -> Result<(), CaptureOwnerRetirementFailure> {
        let mut retention = CaptureBundleRetention::from_owner(self);
        match retention.retire_inner(proof) {
            Ok(()) => Ok(()),
            Err(cause) => {
                Err(CaptureOwnerRetirementFailure { owner: retention.into_owner(), cause })
            }
        }
    }
    /// Borrow the opaque original target for independent cleanup admission.
    pub fn original_origin(&self) -> &CaptureBundleOrigin {
        self.original.original()
    }
}
fn missing() -> ClaimError {
    ClaimError::capture_allocation(CaptureCapacityFailure::missing())
}
impl Drop for CaptureBundleOwner {
    fn drop(&mut self) {
        if let Some(fields) = self.fields.take() {
            let uncertain = fields.files.iter().flatten().any(NativeFile::has_output)
                || fields.directories.iter().flatten().any(|cell| cell.handle.is_armed())
                || fields.token.as_ref().is_some_and(|cell| cell.handle.is_armed())
                || fields.crypto.as_ref().is_some_and(CryptoOwner::has_output);
            if uncertain {
                std::mem::forget(fields);
                if let Some(allocation) = self.allocation.take() {
                    std::mem::forget(allocation);
                }
                std::mem::forget(self.original.clone());
                tracing::error!(
                    operation = "capture_bundle_retained",
                    "Selected release native custody retained"
                );
            } else {
                drop(fields);
            }
        }
    }
}

#[cfg(test)]
mod tests;
