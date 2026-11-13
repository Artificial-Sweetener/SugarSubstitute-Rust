// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain the authenticated current image and PDB under genuine paired symbol execution.
use super::*;
use std::path::Path;
use sugarsubstitute_execution::{
    SymbolBundleAdmission, SymbolBundleOrigin, SymbolBundleRetirement,
};

/// Immutable matching facts from authenticated bytes and the same actual file pair.
#[derive(Clone, Copy, Debug)]
pub struct SymbolRegistrationFacts {
    build: [u8; 64],
    build_length: u8,
    guid: [u8; 16],
    age: u32,
    image_size: u32,
    image_hash: [u8; 32],
    pdb_hash: [u8; 32],
}
impl SymbolRegistrationFacts {
    pub(super) fn new(
        build: &str,
        pair: &super::super::manifest::Pair<'_>,
    ) -> Result<Self, ClaimError> {
        if build.is_empty() || build.len() > 64 {
            return Err(missing());
        }
        let mut text = [0; 64];
        text[..build.len()].copy_from_slice(build.as_bytes());
        Ok(Self {
            build: text,
            build_length: build.len() as u8,
            guid: pair.pdb_guid,
            age: pair.pdb_age,
            image_size: pair.image_size,
            image_hash: pair.pe_sha256,
            pdb_hash: pair.pdb_sha256,
        })
    }
    /// Borrow bounded authenticated build identity without allocating.
    pub fn build(&self) -> Result<&str, ClaimError> {
        std::str::from_utf8(&self.build[..usize::from(self.build_length)]).map_err(|_| missing())
    }
    /// Match the original PE RSDS and same-file PDB information stream.
    pub const fn pdb_guid(&self) -> [u8; 16] {
        self.guid
    }
    /// Match the authenticated PDB age.
    pub const fn pdb_age(&self) -> u32 {
        self.age
    }
    /// Return the authenticated and actually checked PE virtual image size in bytes.
    pub const fn image_size(&self) -> u32 {
        self.image_size
    }
    /// Return the actually checked whole image digest.
    pub const fn image_sha256(&self) -> [u8; 32] {
        self.image_hash
    }
    /// Return the actually checked whole PDB digest.
    pub const fn pdb_sha256(&self) -> [u8; 32] {
        self.pdb_hash
    }
}

/// Preserve the actual consuming admission after a pre-effect construction rejection.
pub struct SymbolConstructionFailure {
    /// The original bounded admission or layout cause.
    pub cause: ClaimError,
    original: SymbolBundleOrigin,
}
impl fmt::Debug for SymbolConstructionFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SymbolConstructionFailure")
            .field("cause", &self.cause)
            .finish_non_exhaustive()
    }
}
impl fmt::Display for SymbolConstructionFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.cause.fmt(f)
    }
}
impl Error for SymbolConstructionFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.cause)
    }
}
impl SymbolConstructionFailure {
    /// Borrow the actual original target for paired independent cleanup.
    pub fn original_origin(&self) -> &SymbolBundleOrigin {
        &self.original
    }
}

/// Own all possible partial outputs before selected symbol file or CNG effects.
pub struct SymbolBundleOwner {
    fields: Option<Box<Fields>>,
    allocation: Option<SharedCaptureLoan>,
    symbol: SymbolBundleOrigin,
}
/// Return the same physical owner and original cause after uncertain checked cleanup.
pub struct SymbolOwnerRetirementFailure {
    /// The identical outstanding native capsule.
    pub owner: SymbolBundleOwner,
    /// The first genuine native or original cleanup-control failure.
    pub cause: ClaimError,
}
impl SymbolBundleOwner {
    /// Consume a genuine once-issued Core symbol admission and inert selection request.
    /// # Errors
    /// Rejects actual source layouts before effects, retaining the original admitted claim.
    pub fn new(
        _request: crate::SymbolSelectionRequest,
        admission: SymbolBundleAdmission,
    ) -> Result<Self, SymbolConstructionFailure> {
        let symbol = admission.into_origin();
        let authority = super::super::ReleaseAuthority::Symbol(symbol.clone());
        if let Err(cause) = capacity::preflight_authority(&authority) {
            return Err(SymbolConstructionFailure {
                cause: ClaimError::capture_allocation(cause),
                original: symbol,
            });
        }
        let original = AllocationOrigin::from_authority(authority).map_err(|failure| {
            SymbolConstructionFailure {
                cause: ClaimError::capture_allocation(failure.cause),
                original: symbol.clone(),
            }
        })?;
        let (fields, allocation) = construct_fields(&original)
            .map_err(|cause| SymbolConstructionFailure { cause, original: symbol.clone() })?;
        Ok(Self { fields: Some(fields), allocation: Some(allocation), symbol })
    }
    /// Authenticate and select once through the sole shared selected-release algorithms.
    /// # Errors
    /// Partial/native/matching failures stay in the same durable owner; selection never retries.
    pub fn select(&mut self) -> Result<(), ClaimError> {
        let original = self.allocation.as_ref().ok_or_else(missing)?.original();
        let fields = self.fields.as_mut().ok_or_else(missing)?;
        if fields.attempted {
            return Err(fields.failure.clone().unwrap_or_else(missing));
        }
        fields.attempted = true;
        let result = selection::select(fields, original, selection::BundlePurpose::Symbol);
        if let Err(cause) = &result {
            fields.failure = Some(cause.clone());
        }
        result
    }
    /// Consume only an actually verified current image/PDB into its trusted retention.
    /// # Errors
    /// An unverified or failed owner returns identical physical custody.
    pub fn take_verified(self) -> Result<SymbolBundleLease, Self> {
        if self
            .fields
            .as_ref()
            .is_some_and(|fields| fields.symbol_facts.is_some() && fields.failure.is_none())
        {
            Ok(SymbolBundleLease { owner: self })
        } else {
            Err(self)
        }
    }
    /// Borrow the actual original symbol target without exposing memory or output factories.
    pub fn original_origin(&self) -> &SymbolBundleOrigin {
        &self.symbol
    }
    /// Settle each actual output under the same independently activated held-cleanup proof.
    /// # Errors
    /// Failure returns the complete identical owner and original cause.
    pub fn retire(
        mut self,
        proof: &SymbolBundleRetirement,
    ) -> Result<(), SymbolOwnerRetirementFailure> {
        let result = retire_fields(
            self.fields.as_mut(),
            self.allocation.as_ref().map(SharedCaptureLoan::original),
            &crate::installed_release::SelectedRetirement::Symbol(proof),
        );
        match result {
            Ok(()) => Ok(()),
            Err(cause) => Err(SymbolOwnerRetirementFailure { owner: self, cause }),
        }
    }
}
impl Drop for SymbolBundleOwner {
    fn drop(&mut self) {
        drop_physical(&mut self.fields, &mut self.allocation);
    }
}

/// Keep same-inode read-only image/PDB and namespace pins across actual DbgHelp use.
pub struct SymbolBundleLease {
    owner: SymbolBundleOwner,
}
impl SymbolBundleLease {
    /// Borrow the actual original target for paired cleanup admission.
    pub fn original_origin(&self) -> &SymbolBundleOrigin {
        self.owner.original_origin()
    }
    /// Revalidate actual original admission before a fresh symbol-provider effect.
    /// # Errors
    /// Rejects terminal/cancelled/expired work while all existing pins remain owned.
    pub fn check_effect(&self) -> Result<(), sugarsubstitute_contracts::execution::AdmissionError> {
        self.owner.symbol.check_effect()
    }
    /// Borrow already authenticated fixed matching facts.
    pub fn registration_facts(&self) -> Result<&SymbolRegistrationFacts, ClaimError> {
        self.owner
            .fields
            .as_ref()
            .and_then(|fields| fields.symbol_facts.as_ref())
            .ok_or_else(missing)
    }
    /// Borrow the canonical current image name while its original file/ancestors stay pinned.
    pub fn image_path(&self) -> Result<&Path, ClaimError> {
        self.path(0)
    }
    /// Borrow the authenticated paired PDB name while its original file stays pinned.
    pub fn pdb_path(&self) -> Result<&Path, ClaimError> {
        self.path(3)
    }
    fn path(&self, index: usize) -> Result<&Path, ClaimError> {
        self.owner
            .fields
            .as_ref()
            .and_then(|fields| fields.files[index].as_ref())
            .ok_or_else(missing)?
            .fields()?
            .path
            .path()
    }
    /// Retire the same retained pair after actual provider closure under paired cleanup.
    /// # Errors
    /// Failure returns identical retention and original cause without renewed admission.
    pub fn retire(
        self,
        proof: &SymbolBundleRetirement,
    ) -> Result<(), SymbolOwnerRetirementFailure> {
        self.owner.retire(proof)
    }
}

#[cfg(test)]
mod tests;
