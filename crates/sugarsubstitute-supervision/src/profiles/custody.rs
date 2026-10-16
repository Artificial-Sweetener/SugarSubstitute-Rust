// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Advance genuine profile preparation and actual Held activation with original custody receipts.
pub(super) mod acknowledgement;
pub(super) mod preparation;
mod recovery;
use super::*;
use crate::registry::profile::prepared::PreparedDesktop;
pub(super) struct Release {
    pub(super) parent: crate::BackendCustodyParent,
    pub(super) key: sugarsubstitute_contracts::backend_custody::ProfileCustodyKey,
    pub(super) until: Option<sugarsubstitute_contracts::execution::Moment>,
    pub(super) nonce: Option<u64>,
    pub(super) counter: Option<(i64, i64)>,
    pub(super) ack: Option<crate::BackendCustodyAcknowledgement>,
}
use sugarsubstitute_contracts::backend_custody::{
    BackendCustodyCommand as Command, BackendCustodyReceipt as Receipt,
};
impl WindowsProfileLease {
    /// Borrow an already-issued helper release transaction without sampling or admitting work.
    /// The exact counter cutoff and nonce remain in the original profile allocation.
    pub fn custody_release_pending(&self) -> bool {
        lock(&self.shared.native)
            .as_ref()
            .and_then(|state| state.custody_release.as_ref())
            .is_some_and(|release| release.until.is_some() && release.ack.is_none())
    }
    /// Observe the exact Published receipt before the caller consumes any launch pins.
    /// # Errors
    /// Preserves authentic transport/correlation failures; pending returns false.
    pub fn custody_publication_ready(&self) -> Result<bool, ClaimError> {
        let mut native = lock(&self.shared.native);
        let state = native.as_mut().ok_or_else(|| ClaimError::invalid("profile_custody_state"))?;
        let held = state.held;
        let Some(prepared) = state.desktop_custody.as_mut() else {
            return Ok(true);
        };
        if prepared.published_ack.is_some() {
            return Ok(true);
        }
        prepared
            .parent
            .consume(
                std::num::NonZeroUsize::new(32)
                    .ok_or_else(|| ClaimError::invalid("profile_published_bound"))?,
            )
            .map_err(|cause| ClaimError::new("profile_published_observation", cause))?;
        if prepared.published_nonce.is_none() && held {
            prepared.published_nonce = prepared
                .parent
                .try_submit(Command::ProfilePublished { profile: prepared.key }, prepared.until)
                .map_err(|cause| ClaimError::new("profile_published_deferred_admission", cause))?;
        }
        acknowledgement::published(
            &prepared.parent,
            prepared.key,
            prepared.published_nonce,
            &mut prepared.published_ack,
        )?;
        Ok(prepared.published_ack.is_some())
    }
    /// Observe actual original registry watch retirement for an unlaunched prepared family.
    /// # Errors
    /// Rejects a foreign registry and preserves the original unbound watch until terminal.
    pub fn retire_prepared_family(
        &self,
        registry: &mut WindowsProcessRegistry,
    ) -> Result<bool, ClaimAdmissionError> {
        self.registry_matches(registry)?;
        if matches!(self.launch, LaunchState::Unattempted | LaunchState::Rejected) {
            registry.retire_prepared_profile(self.run).map_err(ClaimAdmissionError::Registry)
        } else {
            Ok(true)
        }
    }
    /// Observe actual helper release independently from the completed finite close job.
    pub(super) fn advance_custody_release(&self) -> Result<(), ClaimError> {
        let mut native = lock(&self.shared.native);
        let Some(state) = native.as_mut() else {
            return Ok(());
        };
        let Some(release) = state.custody_release.as_mut() else {
            return Ok(());
        };
        if release.ack.is_some() {
            return Ok(());
        }
        let observed = release
            .parent
            .consume(
                std::num::NonZeroUsize::new(32)
                    .ok_or_else(|| ClaimError::invalid("profile_release_bound"))?,
            )
            .map_err(|cause| ClaimError::new("profile_release_observation", cause));
        let receipts = state.desktop_custody.as_mut().map_or(Ok(()), |prepared| {
            let preparation = acknowledgement::prepared_during_close(
                &prepared.parent,
                prepared.key,
                prepared.prepared_nonce,
                &self.shared.closing,
                &mut prepared.prepared_ack,
            );
            let publication = acknowledgement::published(
                &prepared.parent,
                prepared.key,
                prepared.published_nonce,
                &mut prepared.published_ack,
            );
            ClaimError::finish(preparation, publication)
        });
        ClaimError::finish(observed, receipts)?;
        if release.nonce.is_none()
            && let (Some(until), Some((until_ticks, frequency))) = (release.until, release.counter)
        {
            release.nonce = release
                .parent
                .try_submit(
                    Command::ProfileStop { profile: release.key, until_ticks, frequency },
                    until,
                )
                .map_err(|cause| ClaimError::new("profile_release_deferred_admission", cause))?;
        }
        let Some(nonce) = release.nonce else { return Ok(()) };
        if let Some(ack) = release.parent.take_acknowledgement_for(nonce) {
            if ack.receipt() != Receipt::ProfileReleased(release.key) {
                return Err(ClaimError::invalid("profile_release_acknowledgement"));
            }
            release.ack = Some(ack);
            tracing::info!(operation="profile_release_acknowledged",profile=?release.key,nonce,
                "Actual helper settled the original profile custody family and handles");
            if state.retired && state.desktop_custody.is_none() {
                self.shared.active.store(RELEASED, Ordering::Release);
            }
        }
        Ok(())
    }
    /// Prepare actual empty desktop Job and current profile guards before Held publication.
    /// # Errors
    /// Retains partial outputs and original causes through the existing profile owner.
    pub fn submit_custodied_acquire(
        &mut self,
        execution: &ExecutionClient,
        registry: &mut WindowsProcessRegistry,
        scope: &Scope,
        spec: JobSpec,
        parent: &crate::BackendCustodyParent,
        watch: (sugarsubstitute_contracts::execution::ScopeSpec, JobSpec),
    ) -> Result<JobId, ClaimAdmissionError> {
        self.submit_profile_custodied_acquire(execution, registry, scope, spec, parent, watch, None)
    }
    /// Consume only the authentic original Prepared receipt before admitting Held write.
    /// # Errors
    /// Changed source, key, nonce or cutoff rejects without publication.
    pub fn submit_custody_publication(
        &mut self,
        execution: &ExecutionClient,
        scope: &Scope,
        spec: JobSpec,
    ) -> Result<Option<JobId>, ClaimAdmissionError> {
        self.validate_submission(scope, spec)?;
        if self.shared.closing.load(Ordering::Acquire) {
            return Err(ClaimAdmissionError::Closed);
        }
        let mut native = lock(&self.shared.native);
        let state = native.as_mut().ok_or(ClaimAdmissionError::Closed)?;
        let prepared = state.desktop_custody.as_mut().ok_or(ClaimAdmissionError::Identity)?;
        if let Some(command) = prepared.queued_prepare {
            prepared
                .parent
                .consume(std::num::NonZeroUsize::new(32).ok_or(ClaimAdmissionError::Specification)?)
                .map_err(|cause| {
                    ClaimAdmissionError::Native(ClaimError::new(
                        "profile_prepare_deferred_observation",
                        cause,
                    ))
                })?;
            let Some(nonce) =
                prepared.parent.try_submit(command, prepared.until).map_err(|cause| {
                    ClaimAdmissionError::Native(ClaimError::new(
                        "profile_prepare_deferred_admission",
                        cause,
                    ))
                })?
            else {
                return Ok(None);
            };
            prepared
                .destination
                .as_mut()
                .ok_or(ClaimAdmissionError::Identity)?
                .bind_nonce(nonce)
                .map_err(|cause| {
                ClaimAdmissionError::Native(ClaimError::new(
                    "profile_prepare_deferred_nonce",
                    cause,
                ))
            })?;
            prepared.prepared_nonce = Some(nonce);
            prepared.queued_prepare = None;
        }
        if prepared.prepared_ack.is_none() {
            prepared
                .parent
                .consume(std::num::NonZeroUsize::new(32).ok_or(ClaimAdmissionError::Specification)?)
                .map_err(|cause| {
                    ClaimAdmissionError::Native(ClaimError::new(
                        "profile_prepared_observation",
                        cause,
                    ))
                })?;
            let nonce = prepared.prepared_nonce.ok_or(ClaimAdmissionError::Busy)?;
            let Some(ack) = prepared.parent.take_acknowledgement_for(nonce) else {
                return Ok(None);
            };
            if ack.receipt() != Receipt::ProfilePrepared(prepared.key)
                || spec.deadline.is_none_or(|until| until > prepared.until)
            {
                return Err(ClaimAdmissionError::Identity);
            }
            prepared
                .destination
                .as_mut()
                .ok_or(ClaimAdmissionError::Identity)?
                .acknowledge(&ack)
                .map_err(|cause| {
                ClaimAdmissionError::Native(ClaimError::new("profile_prepared_ack", cause))
            })?;
            prepared.prepared_ack = Some(ack);
            tracing::info!(operation="profile_prepared_acknowledged",profile=?prepared.key,nonce,
                "Actual helper retained the prepared empty desktop Job and independent profile exclusion");
        }
        drop(native);
        self.submit(execution, scope, spec, publish, false)?;
        Ok(Some(spec.id))
    }
}
fn publish(
    state: &mut storage::NativeState,
    context: &sugarsubstitute_contracts::execution::JobContext,
) -> Result<(), ClaimError> {
    storage::check(context)?;
    super::migration::preserve(state, context)?;
    let prepared = state
        .desktop_custody
        .as_mut()
        .ok_or_else(|| ClaimError::invalid("profile_custody_prepared"))?;
    if prepared
        .prepared_ack
        .as_ref()
        .is_none_or(|ack| ack.receipt() != Receipt::ProfilePrepared(prepared.key))
    {
        return Err(ClaimError::invalid("profile_custody_publication_ack"));
    }
    prepared
        .parent
        .check()
        .map_err(|cause| ClaimError::new("profile_custody_publication_parent", cause))?;
    let record =
        state.custody_record.ok_or_else(|| ClaimError::invalid("profile_custody_record"))?;
    let file = state.file.as_ref().ok_or_else(|| ClaimError::invalid("profile_custody_writer"))?;
    state.publication_attempted = true;
    crate::protected_storage::record_file::publish(
        file,
        &record.encode(),
        crate::protected_storage::record_file::PublicationAuthority::Ordinary,
        context,
        |old| match old {
            crate::protected_storage::record_file::RecordRead::Missing if file.created => Ok(()),
            crate::protected_storage::record_file::RecordRead::Present(bytes)
                if state.original_released.as_ref() == Some(bytes) =>
            {
                Ok(())
            }
            _ => Err(ClaimError::invalid("profile_custody_changed_predecessor")),
        },
    )?;
    state.record = Some(record.original);
    state.held = true;
    prepared.published_nonce = prepared
        .parent
        .try_submit(Command::ProfilePublished { profile: prepared.key }, prepared.until)
        .map_err(|cause| ClaimError::new("profile_published_admission", cause))?;
    Ok(())
}
