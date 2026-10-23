// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Transfer actually settled provisioning resources into execution-owned cache finalization.
use super::*;
use crate::registry_input::preparation::PreparationPayload;
use std::io;

/// Original verified source, cache grants and native allocation after complete family retirement.
/// Only the registry creates this result. Cancellation and supplied exit codes cannot create it.
/// Dropping preserves durable recovery facts; it performs no cache cleanup.
pub struct SettledProvisioning {
    family: native_process::FamilyStorage,
}
impl SettledProvisioning {
    /// Inventory stopped writers and release their immutable source reference on a storage worker.
    /// Reserve 8 MiB alongside the original retained scope. Keep this result on failure and retry
    /// finalization with a fresh current job; a successful earlier inventory is idempotent.
    /// # Errors
    /// Preserves cancellation, unknown control files and actual native/storage retirement failures.
    pub fn finish(&mut self, context: &JobContext) -> io::Result<()> {
        let launch = self
            .family
            .preparation_image
            .as_mut()
            .ok_or_else(|| io::Error::other("Settled provisioning resources are absent"))?;
        let PreparationPayload::Provisioning(provisioning) = &mut launch.payload else {
            return Err(io::Error::other("Settled provisioning role differs"));
        };
        provisioning.finish_settled(context)
    }
}
impl WindowsProcessRegistry {
    /// Transfer resources only after actual family, process watches and creation/cleanup jobs end.
    /// The original native allocation and cache pins remain owned through finalization.
    /// # Errors
    /// Rejects foreign/stale keys, outstanding observations and incomplete or unrelated families.
    pub fn take_settled_provisioning(
        &mut self,
        key: ProcessKey,
    ) -> Result<SettledProvisioning, RegistryAdmissionError> {
        let index = self.index(key)?;
        let record = self.records[index].as_mut().ok_or(RegistryAdmissionError::Stale)?;
        if !record.settled
            || record.creation.is_some()
            || record.cleanup.is_some()
            || record.root_watch.is_some()
            || record.member_watch.is_some()
            || record.establishment_job.is_some()
            || record.unread.iter().any(|value| *value)
        {
            return Err(RegistryAdmissionError::Busy);
        }
        let mut state = lock(&record.shared.native);
        let family = state.family.as_ref().ok_or(RegistryAdmissionError::Closed)?;
        if family.has_native_ownership() || !family.closure_committed {
            return Err(RegistryAdmissionError::Busy);
        }
        if family.preparation_image.as_ref().and_then(|launch| launch.provisioning()).is_none() {
            return Err(RegistryAdmissionError::InvalidSpecification);
        }
        let family = state.family.take().ok_or(RegistryAdmissionError::Closed)?;
        Ok(SettledProvisioning { family })
    }
}
