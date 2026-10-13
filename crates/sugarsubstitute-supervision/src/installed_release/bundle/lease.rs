// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Transfer verified pins and original module credits into the sole capture loader.
use super::*;
use std::path::Path;
use sugarsubstitute_execution::CaptureBundleRetirement;
/// Bounded authenticated registration facts; data alone grants no native effect.
#[derive(Clone, Copy, Debug)]
pub struct CaptureRegistrationFacts {
    build: [u8; 64],
    length: u8,
    client: [u8; 32],
    handler: [u8; 32],
}
impl CaptureRegistrationFacts {
    pub(super) fn new(
        build: &str,
        client: [u8; 32],
        handler: [u8; 32],
    ) -> Result<Self, ClaimError> {
        if build.is_empty() || build.len() > 64 {
            return Err(missing());
        }
        let mut bytes = [0; 64];
        bytes[..build.len()].copy_from_slice(build.as_bytes());
        Ok(Self { build: bytes, length: build.len() as u8, client, handler })
    }
    /// Borrow authenticated bounded build text without allocation.
    pub fn build(&self) -> Result<&str, ClaimError> {
        std::str::from_utf8(&self.build[..usize::from(self.length)]).map_err(|_| missing())
    }
    /// Match the exact already authenticated client file.
    pub fn client_sha256(&self) -> [u8; 32] {
        self.client
    }
    /// Match the exact already authenticated handler file.
    pub fn handler_sha256(&self) -> [u8; 32] {
        self.handler
    }
}
/// Consume a verified owner; no public constructor can forge this lease.
pub struct CaptureBundleLease {
    pub(super) owner: CaptureBundleOwner,
}
/// Transfer the same file retention and both pre-reserved module output cells.
pub struct CaptureBundleParts {
    /// The same authenticated file, namespace and original-budget retention.
    pub retention: CaptureBundleRetention,
    /// The first pre-reserved loaded module output.
    pub loaded: CaptureModuleOutput,
    /// The independently pre-reserved permanent module pin output.
    pub pinned: CaptureModuleOutput,
}
/// Keep actual no-write/no-delete pins through every loader/handler effect.
pub struct CaptureBundleRetention {
    owner: CaptureBundleOwner,
}
/// Preserve the identical physical retention after a failed checked retirement.
pub struct CaptureRetirementFailure {
    /// The identical actual pins and outstanding output cells.
    pub retention: CaptureBundleRetention,
    /// The first genuine close or original-control failure.
    pub cause: ClaimError,
}
impl CaptureBundleLease {
    /// Consume into the only actual module/PIN custody; no second quota is admitted.
    /// # Errors
    /// A corrupted internal consuming slot returns the identical lease.
    pub fn into_parts(mut self) -> Result<CaptureBundleParts, Self> {
        let Some(fields) = self.owner.fields.as_mut() else { return Err(self) };
        if fields.modules.iter().any(Option::is_none) {
            return Err(self);
        }
        let loaded = fields.modules[0].take();
        let pinned = fields.modules[1].take();
        match (loaded, pinned) {
            (Some(loaded), Some(pinned)) => Ok(CaptureBundleParts {
                retention: CaptureBundleRetention { owner: self.owner },
                loaded,
                pinned,
            }),
            (loaded, pinned) => {
                if let Some(fields) = self.owner.fields.as_mut() {
                    fields.modules = [loaded, pinned];
                }
                Err(self)
            }
        }
    }
}
impl CaptureBundleRetention {
    pub(super) fn from_owner(owner: CaptureBundleOwner) -> Self {
        Self { owner }
    }
    pub(super) fn into_owner(self) -> CaptureBundleOwner {
        self.owner
    }
    /// Borrow the exact original target for actual independent close-job admission.
    pub fn original_origin(&self) -> &CaptureBundleOrigin {
        self.owner.original_origin()
    }
    /// Observe the immutable selected registration facts.
    pub fn registration_facts(&self) -> Result<&CaptureRegistrationFacts, ClaimError> {
        self.owner.fields.as_ref().and_then(|f| f.facts.as_ref()).ok_or_else(missing)
    }
    /// Borrow the canonical protected client name while its actual file remains pinned.
    pub fn client_path(&self) -> Result<&Path, ClaimError> {
        self.path(4)
    }
    /// Borrow the canonical protected handler name while its actual file remains pinned.
    pub fn handler_path(&self) -> Result<&Path, ClaimError> {
        self.path(6)
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
    /// Recheck the original actual target before a fresh loader or handler effect.
    /// # Errors
    /// Rejects terminal/cancelled/expired original admission; retained pins stay owned.
    pub fn check_effect(&self) -> Result<(), sugarsubstitute_contracts::execution::AdmissionError> {
        self.owner.original.original().check_effect()
    }
    /// Retire every existing actual output using the same borrowed cleanup proof.
    /// # Errors
    /// Each fresh close checks its immutable cutoff; failure returns identical retention.
    pub fn retire(
        mut self,
        proof: &CaptureBundleRetirement,
    ) -> Result<(), CaptureRetirementFailure> {
        let result = self.retire_inner(proof);
        match result {
            Ok(()) => Ok(()),
            Err(cause) => Err(CaptureRetirementFailure { retention: self, cause }),
        }
    }
    pub(super) fn retire_inner(
        &mut self,
        proof: &CaptureBundleRetirement,
    ) -> Result<(), ClaimError> {
        if !proof.matches_origin(self.original_origin()) {
            return Err(missing());
        }
        self.owner.original.close().map_err(ClaimError::capture_allocation)?;
        let fields = self.owner.fields.as_mut().ok_or_else(missing)?;
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
}
