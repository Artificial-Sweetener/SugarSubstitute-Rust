// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Transfer the actual prepared Job and pinned profile directory before Held publication.
use super::*;
use crate::{durable_storage::protected::paths, native_claims::files::Budget};
use sugarsubstitute_contracts::execution::JobContext;
/// Retain every partial duplicate in the genuine destination before original command admission.
pub(in crate::profiles) fn prepare(
    state: &mut storage::NativeState,
    record: record::Record,
    context: &JobContext,
) -> Result<(), ClaimError> {
    storage::check(context)?;
    let file = state.file.as_ref().ok_or_else(|| ClaimError::invalid("profile_prepare_writer"))?;
    let budget = Budget::Protected(
        state.memory.as_ref().ok_or_else(|| ClaimError::invalid("profile_prepare_memory"))?.clone(),
    );
    let prepared = state
        .desktop_custody
        .as_mut()
        .ok_or_else(|| ClaimError::invalid("profile_prepare_owner"))?;
    if let Some(grant) = &prepared.recovery_grant {
        grant
            .check_context(context)
            .map_err(|cause| ClaimError::new("recovery_profile_prepare_original", cause))?;
        if prepared.key.run() != grant.key().run() {
            return Err(ClaimError::invalid("recovery_profile_prepare_binding"));
        }
    }
    let original =
        record::custody::CustodiedRecord::new(record, prepared.parent.key(), prepared.key)?;
    let until = prepared
        .client
        .encode_lifecycle_deadline(prepared.until)
        .map_err(|cause| ClaimError::new("profile_custody_prepare_counter", cause))?;
    let root = state.root.as_ref().ok_or_else(|| ClaimError::invalid("profile_custody_root"))?;
    if let Some(grant) = &prepared.recovery_grant {
        let (_, _, sid) = root.profile_folder()?;
        prepared
            .family
            .as_mut()
            .ok_or_else(|| ClaimError::invalid("recovery_profile_family"))?
            .secure_recovery_inheritance(sid, &budget, grant, context)
            .map_err(|cause| ClaimError::new("recovery_profile_job_security", cause))?;
    }
    let writer = paths::file_identity(
        &file.handle,
        &mut crate::durable_storage::protected::native_phase::NativePhase::ordinary(&budget)?,
    )?;
    let target = prepared
        .destination
        .as_mut()
        .ok_or_else(|| ClaimError::invalid("profile_custody_destination"))?;
    target.open().map_err(|cause| ClaimError::new("profile_custody_destination", cause))?;
    let job = target
        .duplicate(
            crate::native_process::handle(
                prepared
                    .family
                    .as_ref()
                    .ok_or_else(|| ClaimError::invalid("profile_custody_family"))?
                    .job,
                "profile_custody_job",
            )
            .map_err(|cause| ClaimError::new("profile_custody_job", cause))?,
        )
        .map_err(|cause| ClaimError::new("profile_custody_duplicate_job", cause))?;
    let directory = target
        .duplicate(root.profile_handle()?)
        .map_err(|cause| ClaimError::new("profile_custody_duplicate_directory", cause))?;
    let command =
        sugarsubstitute_contracts::backend_custody::BackendCustodyCommand::ProfilePrepare {
            recovery_grant: prepared.recovery_grant.as_ref().map(|grant| grant.key().id()),
            profile: prepared.key,
            job,
            directory,
            record: original.encode(),
            claim_identity: writer,
            until_ticks: until.ticks(),
            frequency: until.frequency(),
        };
    prepared.queued_prepare = Some(command);
    if let Some(nonce) = prepared
        .parent
        .try_submit(command, prepared.until)
        .map_err(|cause| ClaimError::new("profile_custody_prepare_admission", cause))?
    {
        target
            .bind_nonce(nonce)
            .map_err(|cause| ClaimError::new("profile_custody_prepare_nonce", cause))?;
        prepared.prepared_nonce = Some(nonce);
        prepared.queued_prepare = None;
    }
    state.custody_record = Some(original);
    storage::check(context)
}
