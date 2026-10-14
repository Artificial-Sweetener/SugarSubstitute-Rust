// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own selected native outputs before effects and retain the complete uncertain capsule.
mod capacity;
mod files;
mod lease;
mod selection;
mod symbol;
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
pub use symbol::{
    SymbolBundleLease, SymbolBundleOwner, SymbolConstructionFailure, SymbolOwnerRetirementFailure,
    SymbolRegistrationFacts,
};

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
    symbol_facts: Option<SymbolRegistrationFacts>,
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
    capture: CaptureBundleOrigin,
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
        let retained = origin.clone();
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
            Err(cause) => Err(CaptureConstructionFailure { cause, original: retained }),
        }
    }
    fn construct(original: AllocationOrigin) -> Result<Self, ClaimError> {
        let capture = original.original().capture().ok_or_else(missing)?.clone();
        let (fields, allocation) = construct_fields(&original)?;
        Ok(Self { capture, fields: Some(fields), allocation: Some(allocation) })
    }
    /// Select once; native and matching failures remain with the same durable owner.
    /// # Errors
    /// Fails closed without a compiled signer or genuine same-file matching association.
    pub fn select(&mut self) -> Result<(), ClaimError> {
        let original = self.allocation.as_ref().ok_or_else(missing)?.original();
        let fields = self.fields.as_mut().ok_or_else(missing)?;
        if fields.attempted {
            return Err(fields.failure.clone().unwrap_or_else(missing));
        }
        fields.attempted = true;
        let result = selection::select(fields, original, selection::BundlePurpose::Capture);
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
        &self.capture
    }
}
/// Allocate the one shared physical graph only after its sealed origin is preflighted.
fn construct_fields(
    original: &AllocationOrigin,
) -> Result<(Box<Fields>, SharedCaptureLoan), ClaimError> {
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
        symbol_facts: None,
        attempted: false,
        failure: None,
    });
    let causes = |slot: u8| -> Result<NativeCauseDestinations, ClaimError> {
        Ok(NativeCauseDestinations::selected(
            original,
            original.cause_destination(slot * 2).map_err(ClaimError::capture_allocation)?,
            original.cause_destination(slot * 2 + 1).map_err(ClaimError::capture_allocation)?,
        ))
    };
    for index in 0..80 {
        let slot = if index < 40 { index } else { index + 9 };
        let mut causes = causes(slot as u8)?;
        let output = fields.quota.reserve_selected(&mut causes)?;
        let handle = Handle::reserve_capture(&mut Some((original.clone(), output)))?;
        fields.directories[index] = Some(OwnedHandle { handle, causes, identity: None });
    }
    let mut token_causes = causes(48)?;
    let output = fields.quota.reserve_selected(&mut token_causes)?;
    let handle = Handle::reserve_capture(&mut Some((original.clone(), output)))?;
    fields.token = Some(OwnedHandle { handle, causes: token_causes, identity: None });
    let file_count = if original.original().symbol().is_some() { 4 } else { 8 };
    for index in 0..file_count {
        let mut causes = causes((40 + index) as u8)?;
        let output = fields.quota.reserve_selected(&mut causes)?;
        fields.prepared[index] = Some(PreparedFile { output, causes });
    }
    let mut provider_causes = causes(89)?;
    let provider = fields.quota.reserve_selected(&mut provider_causes)?;
    let mut key_causes = causes(90)?;
    let key = fields.quota.reserve_selected(&mut key_causes)?;
    fields.crypto = Some(CryptoOwner::selected(
        original.clone(),
        provider,
        key,
        [provider_causes, key_causes],
    )?);
    if original.original().capture().is_some() {
        for index in 0..2 {
            let mut causes = causes((91 + index) as u8)?;
            let output = fields.quota.reserve_selected(&mut causes)?;
            fields.modules[index] = Some(CaptureModuleOutput::from_guard(output, causes));
        }
    }
    for index in 0..4 {
        fields.causes[index] = Some(causes((96 + index) as u8)?);
    }
    Ok((fields, outside))
}

/// Check the same cleanup proof before each fresh close; late correlated return is passive.
fn retire_fields(
    fields: Option<&mut Box<Fields>>,
    origin: Option<&AllocationOrigin>,
    proof: &crate::installed_release::SelectedRetirement<'_>,
) -> Result<(), ClaimError> {
    let origin = origin.ok_or_else(missing)?;
    if !proof.matches_origin(origin.original()) {
        return Err(missing());
    }
    origin.close().map_err(ClaimError::capture_allocation)?;
    let fields = fields.ok_or_else(missing)?;
    for file in fields.files.iter_mut().flatten() {
        file.retire_selected(proof)?;
    }
    for cell in fields.directories.iter_mut().flatten() {
        cell.handle.close_selected(proof, &mut cell.causes)?;
    }
    if let Some(token) = fields.token.as_mut() {
        token.handle.close_selected(proof, &mut token.causes)?;
    }
    if let Some(crypto) = fields.crypto.as_mut() {
        crypto.retire_capture(proof)?;
    }
    Ok(())
}

fn missing() -> ClaimError {
    ClaimError::capture_allocation(CaptureCapacityFailure::missing())
}
impl Drop for CaptureBundleOwner {
    fn drop(&mut self) {
        drop_physical(&mut self.fields, &mut self.allocation);
    }
}
fn drop_physical(fields: &mut Option<Box<Fields>>, allocation: &mut Option<SharedCaptureLoan>) {
    if let Some(fields) = fields.take() {
        let uncertain = fields.files.iter().flatten().any(NativeFile::has_output)
            || fields.directories.iter().flatten().any(|cell| cell.handle.is_armed())
            || fields.token.as_ref().is_some_and(|cell| cell.handle.is_armed())
            || fields.crypto.as_ref().is_some_and(CryptoOwner::has_output);
        if uncertain {
            std::mem::forget(fields);
            if let Some(allocation) = allocation.take() {
                std::mem::forget(allocation);
            }
            tracing::error!(
                operation = "capture_bundle_retained",
                "Selected release native custody retained"
            );
        } else {
            drop(fields);
        }
    }
}

#[cfg(test)]
mod tests;
