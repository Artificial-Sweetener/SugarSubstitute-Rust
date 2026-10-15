// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Transfer the actual prepared Job and pinned profile directory before Held publication.
use super::*;
use crate::native_claims::files::Budget;
use crate::protected_storage::paths;
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
    let original =
        record::custody::CustodiedRecord::new(record, prepared.parent.key(), prepared.key)?;
    let until = prepared
        .client
        .encode_lifecycle_deadline(prepared.until)
        .map_err(|cause| ClaimError::new("profile_custody_prepare_counter", cause))?;
    let root = state.root.as_ref().ok_or_else(|| ClaimError::invalid("profile_custody_root"))?;
    let writer = paths::file_identity(
        &file.handle,
        &mut crate::protected_storage::native_phase::NativePhase::ordinary(&budget)?,
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
