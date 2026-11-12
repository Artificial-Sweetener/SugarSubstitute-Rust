// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Transfer actual settled probe custody into durable publication without releasing its original charge.
use super::*;
use crate::{
    registry_input::preparation::PreparationPayload,
    setup_storage::{PreparationImage, TrustFailure, UpdaterProbeDisposition},
};

/// Unique completed native probe and its original authenticated trial, payload and exclusion.
/// Only the registry can create this result after all native work and observations settle.
/// Dropping it preserves the pending journal for fresh exclusive recovery; it never implies health.
pub struct SettledUpdaterProbe {
    family: native_process::FamilyStorage,
    disposition: UpdaterProbeDisposition,
}
impl SettledUpdaterProbe {
    /// Publish the exact observed result on an admitted storage worker. Retain this owner on error
    /// and retry the same decision. Successful publication is idempotent and retains custody until
    /// this value is dropped. `now` returns Unix UTC seconds for current signed authorization.
    /// # Errors
    /// Rejects stale journals, changed trust, expired acceptance authority, cancellation or I/O failure.
    pub fn publish(
        &mut self,
        now: impl FnMut() -> i64,
        context: &JobContext,
    ) -> Result<(), TrustFailure> {
        let Some(launch) = self.family.preparation_image.as_mut() else {
            return Err(std::io::Error::other("Settled updater custody is absent").into());
        };
        let PreparationPayload::Probe(probe) = &mut launch.payload else {
            return Err(std::io::Error::other("Settled updater role differs").into());
        };
        probe.publish_settled(self.disposition, now, context)
    }

    /// Independently pin this successfully probed engine after durable acceptance. Run on an
    /// admitted storage worker with payload plus trust-operation capacity. The original trust
    /// workspace is reused; no second store or application lease is acquired. Retain this result
    /// on error for retry, and drop it only after the returned image has a new process owner.
    /// The image carries file custody but no preparation exclusion or process-health authority.
    /// # Errors
    /// Rejects unsuccessful or unpublished probes, changed accepted identity, changed payloads,
    /// insufficient admission, cancellation and native storage failure without losing custody.
    pub fn pin_preparation(&self, context: &JobContext) -> Result<PreparationImage, TrustFailure> {
        if !matches!(self.disposition, UpdaterProbeDisposition::Accepted) {
            return Err(std::io::Error::other("Updater probe did not succeed").into());
        }
        let probe = self
            .family
            .preparation_image
            .as_ref()
            .and_then(|launch| launch.probe())
            .ok_or_else(|| std::io::Error::other("Settled updater custody is absent"))?;
        probe.pin_accepted(context)
    }
}
impl WindowsProcessRegistry {
    /// Transfer one probe only after actual family settlement and complete observation retirement.
    /// Cached snapshots, supplied exit codes and elapsed deadlines cannot create this authority.
    /// Retains the original native family allocation, payload pins and exclusion until publication.
    /// # Errors
    /// Rejects foreign/stale keys, incomplete settlement and non-probe or already transferred roles.
    pub fn take_settled_updater_probe(
        &mut self,
        key: ProcessKey,
    ) -> Result<SettledUpdaterProbe, RegistryAdmissionError> {
        let index = self.index(key)?;
        let record = self.records[index].as_mut().ok_or(RegistryAdmissionError::Stale)?;
        if !record.settled
            || record.creation.is_some()
            || record.cleanup.is_some()
            || record.root_watch.is_some()
            || record.member_watch.is_some()
            || record.establishment_job.is_some()
            || record.unread.iter().any(|unread| *unread)
        {
            return Err(RegistryAdmissionError::Busy);
        }
        let mut state = lock(&record.shared.native);
        let family = state.family.as_ref().ok_or(RegistryAdmissionError::Closed)?;
        if family.has_native_ownership() || !family.closure_committed {
            return Err(RegistryAdmissionError::Busy);
        }
        if family.preparation_image.as_ref().and_then(|launch| launch.probe()).is_none() {
            return Err(RegistryAdmissionError::InvalidSpecification);
        }
        let disposition = if !record.started
            && record.root.is_none()
            && family.root.is_none()
            && family.count == 0
        {
            UpdaterProbeDisposition::Unstarted
        } else if record.started
            && record.root_exit.is_some_and(|exit| exit.exit_code == 0)
            && !record.stopped_before_exit
            && !record.forced
            && !record.failure_transferred
            && record.error.is_none()
            && state.failure.is_none()
        {
            UpdaterProbeDisposition::Accepted
        } else {
            UpdaterProbeDisposition::Rejected
        };
        let family = state.family.take().ok_or(RegistryAdmissionError::Closed)?;
        Ok(SettledUpdaterProbe { family, disposition })
    }
}
