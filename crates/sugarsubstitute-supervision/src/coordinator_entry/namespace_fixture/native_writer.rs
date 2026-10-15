// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Discover real token/private-directory outputs while substituting installation/image observations.
//! The Account selector uses this D-local external discovery root; this fixture does not query or
//! redirect the actual Shell known folder. Entry admission, consumption and source custody stay real.
use super::*;
use crate::native_claims::files::{self, Budget};
use crate::protected_storage::{
    account, native_name::NativeNameDestination, native_phase::NativePhase, paths,
};
/// Supply an actual invalid external pin or a normal native observation.
#[derive(Clone, Copy)]
pub(crate) enum PinFault {
    None,
    Closed,
    ForeignMemory,
}
/// Compose real Entry admission with the owned D-native external discovery boundary.
pub(crate) fn owner(
    input: CoordinatorEntryInput,
    budget: EntrySetupBudget,
    root: PathBuf,
    fault: PinFault,
) -> CoordinatorEntryOwner {
    CoordinatorEntryOwner::with_port(input, budget, Arc::new(Boundary { root, fault }))
}
struct Boundary {
    root: PathBuf,
    fault: PinFault,
}
impl NativePort for Boundary {
    fn discover(
        &self,
        state: &mut NativeState,
        _: &Path,
        _: &Path,
        diagnostics: &super::super::DiagnosticStorageLocation,
        context: &JobContext,
    ) -> Result<Discovered, ClaimError> {
        native::check(context)?;
        std::fs::create_dir(&self.root)
            .map_err(|cause| ClaimError::new("writer_fixture_parent", cause))?;
        let memory = state
            .memory
            .as_ref()
            .ok_or_else(|| ClaimError::invalid("writer_fixture_memory"))?
            .clone();
        let budget = Budget::Protected(memory);
        state
            .handles
            .try_reserve_exact(260)
            .map_err(|cause| ClaimError::new("writer_fixture_handles", cause))?;
        let mut phase = NativePhase::ordinary(&budget)?;
        let account = account::resolve_current(
            &mut state.handles,
            budget.clone(),
            account::PrimaryAccountAccess::Query,
            &mut phase,
        )?;
        let parent = paths::open_retained(
            &self.root,
            true,
            true,
            budget.clone(),
            &mut state.handles,
            &mut phase,
        )?;
        let identity = paths::file_identity(&state.handles[parent], &mut phase)?;
        let canonical =
            paths::final_path(&state.handles[parent], &mut phase, NativeNameDestination::Ordinary)?
                .into_ordinary()?;
        let namespace = NamespaceId::new([1; 32]);
        let mut resolved = ResolvedNamespace {
            namespace,
            account_sid: account.sid.into_ordinary()?,
            session_id: account.session_id,
            installation_identity: identity,
            directory: canonical,
            directory_slot: parent,
            directory_identity: identity,
            diagnostics: None,
        };
        account::resolve_diagnostic_root(
            &mut resolved,
            &super::super::DiagnosticStorageLocation::Explicit(self.root.join("account")),
            &mut state.handles,
            budget.clone(),
            context,
        )?;
        let (directory, slot, inode) = resolved.diagnostic_directory();
        let directory = directory.to_owned();
        resolved.directory = directory;
        resolved.directory_slot = slot;
        resolved.directory_identity = inode;
        resolved.diagnostics = None;
        account::resolve_diagnostic_root(
            &mut resolved,
            diagnostics,
            &mut state.handles,
            budget,
            context,
        )?;
        match self.fault {
            PinFault::None => (),
            PinFault::Closed => {
                let slot = resolved.diagnostic_directory().1;
                state.handles[slot].close()?;
            }
            PinFault::ForeignMemory => {
                state.memory =
                    Some(Arc::new(context.memory().claim(0).map_err(|cause| {
                        ClaimError::new("writer_fixture_foreign_memory", cause)
                    })?));
            }
        }
        let facts = sugarsubstitute_contracts::coordinator_entry::EntryNativeIdentityFacts::new(
            namespace,
            &resolved.account_sid,
            resolved.session_id,
            [7; 24],
        )
        .map_err(|cause| ClaimError::new("writer_fixture_facts", cause))?;
        state.namespace = Some(resolved);
        state.image_identity = Some([7; 24]);
        Ok(Discovered { facts, installation_identity: identity })
    }
    fn close(&self, state: &mut NativeState, context: &JobContext) -> Result<(), ClaimError> {
        native::check(context)?;
        files::close_all(&mut state.handles)?;
        state.namespace = None;
        state.memory = None;
        state.image_identity = None;
        state.retired = true;
        Ok(())
    }
    fn elect(&self, _: &mut NativeState, _: &JobContext) -> Result<Elected, ClaimError> {
        Err(ClaimError::invalid("writer_fixture_no_election"))
    }
    fn validate_coordinator(
        &self,
        _: &NativeState,
        _: &Handle,
        _: [u8; 24],
        _: &JobContext,
    ) -> Result<(), ClaimError> {
        Err(ClaimError::invalid("writer_fixture_no_coordinator"))
    }
    fn validate_election(
        &self,
        _: &NativeState,
        _: &RetainedElectionProof,
        _: &JobContext,
    ) -> Result<(), ClaimError> {
        Err(ClaimError::invalid("writer_fixture_no_election"))
    }
}
