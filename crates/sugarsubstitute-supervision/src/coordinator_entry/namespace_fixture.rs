// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Run actual entry discovery and proof consumption over an inert external native boundary.
//! This fixture opens no filesystem or process handle. Callers submit real finite jobs,
//! consume the genuine completion batch, then take the owner's sealed namespace once.
use super::{
    CoordinatorEntryInput, CoordinatorEntryOwner,
    election::RetainedElectionProof,
    namespace::NativeState,
    native::{self, Discovered, Elected, NativePort},
};
use crate::{
    ClaimError, native_claims::files::Handle, protected_storage::account::ResolvedNamespace,
};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use sugarsubstitute_contracts::{
    coordinator_entry::{EntryNativeIdentityFacts, EntrySetupBudget},
    execution::JobContext,
    lifecycle::NamespaceId,
};

/// Compose the authoritative owner with external discovery facts and no native output.
/// Actual execution admission, original deadlines, completion consumption, proof transfer
/// and close remain implemented by `CoordinatorEntryOwner`. The caller supplies every job.
/// The returned namespace supports passive request-custody tests; it qualifies no real path.
pub fn namespace_owner(
    input: CoordinatorEntryInput,
    budget: EntrySetupBudget,
) -> CoordinatorEntryOwner {
    CoordinatorEntryOwner::with_port(input, budget, Arc::new(Boundary))
}
struct Boundary;
impl NativePort for Boundary {
    fn discover(
        &self,
        state: &mut NativeState,
        _: &Path,
        _: &Path,
        _: &super::DiagnosticStorageLocation,
        context: &JobContext,
    ) -> Result<Discovered, ClaimError> {
        native::check(context)?;
        if state.memory.is_none() || !state.handles.is_empty() {
            return Err(ClaimError::invalid("namespace_boundary_missing_original_claim"));
        }
        let namespace = NamespaceId::new([1; 32]);
        let sid = vec![1, 1, 0, 0, 0, 0, 0, 5, 42, 0, 0, 0];
        let facts = EntryNativeIdentityFacts::new(namespace, &sid, 4, [7; 24])
            .map_err(|cause| ClaimError::new("namespace_boundary_facts", cause))?;
        state.namespace = Some(ResolvedNamespace {
            namespace,
            account_sid: sid,
            session_id: 4,
            installation_identity: [6; 24],
            directory: PathBuf::from("entry-boundary-inert"),
            directory_slot: 0,
            directory_identity: [1; 24],
            diagnostics: None,
        });
        state.image_identity = Some([7; 24]);
        Ok(Discovered { facts, installation_identity: [6; 24] })
    }
    fn elect(&self, _: &mut NativeState, context: &JobContext) -> Result<Elected, ClaimError> {
        native::check(context)?;
        Err(ClaimError::invalid("namespace_boundary_has_no_election"))
    }
    fn close(&self, state: &mut NativeState, context: &JobContext) -> Result<(), ClaimError> {
        native::check(context)?;
        if !state.handles.is_empty()
            || state.election.is_some()
            || state.recovery_coordinator.is_some()
        {
            return Err(ClaimError::invalid("namespace_boundary_has_native_custody"));
        }
        state.memory = None;
        state.namespace = None;
        state.image_identity = None;
        state.retired = true;
        Ok(())
    }
    fn validate_coordinator(
        &self,
        _: &NativeState,
        _: &Handle,
        _: [u8; 24],
        context: &JobContext,
    ) -> Result<(), ClaimError> {
        native::check(context)?;
        Err(ClaimError::invalid("namespace_boundary_has_no_coordinator"))
    }
    fn validate_election(
        &self,
        _: &NativeState,
        _: &RetainedElectionProof,
        context: &JobContext,
    ) -> Result<(), ClaimError> {
        native::check(context)?;
        Err(ClaimError::invalid("namespace_boundary_has_no_election"))
    }
}
