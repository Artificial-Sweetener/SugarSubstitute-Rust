// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Execute finite native namespace discovery, stable election and checked retirement.
mod election;
use super::{
    election::{ElectionRecordOrigin, FreshRecordOrigin},
    namespace::NativeState,
};
use crate::{
    ClaimError,
    native_claims::{
        Record, RecordState,
        files::{Budget, Handle},
    },
    protected_storage::{
        account,
        exclusion::HeldFile,
        paths,
        record_file::{self, RecordRead},
        security,
    },
};
use std::{error::Error, path::Path};
use sugarsubstitute_contracts::{
    coordinator_entry::{EntryNativeIdentityFacts, EntryOwnerDegradation},
    execution::JobContext,
    lifecycle::CoordinatorEpoch,
};

#[derive(Clone, Copy)]
pub(super) struct Discovered {
    pub(super) facts: EntryNativeIdentityFacts,
    pub(super) installation_identity: [u8; 24],
}
#[derive(Clone, Copy)]
pub(super) enum Elected {
    Fresh { origin: FreshRecordOrigin, inode: [u8; 24] },
    OrdinaryReleased { record: [u8; 128], inode: [u8; 24] },
    RecoveredHeld { record: [u8; 128], inode: [u8; 24], coordinator_inode: [u8; 24] },
    Candidate(CoordinatorEpoch),
    Contended,
    Degraded(EntryOwnerDegradation),
}
pub(super) trait NativePort: Send + Sync {
    fn discover(
        &self,
        state: &mut NativeState,
        installation: &Path,
        executable: &Path,
        diagnostics: &super::DiagnosticStorageLocation,
        context: &JobContext,
    ) -> Result<Discovered, ClaimError>;
    fn elect(&self, state: &mut NativeState, context: &JobContext) -> Result<Elected, ClaimError>;
    fn close(&self, state: &mut NativeState, context: &JobContext) -> Result<(), ClaimError>;
    fn validate_coordinator(
        &self,
        state: &NativeState,
        handle: &Handle,
        inode: [u8; 24],
        context: &JobContext,
    ) -> Result<(), ClaimError>;
    fn validate_election(
        &self,
        state: &NativeState,
        proof: &super::election::RetainedElectionProof,
        context: &JobContext,
    ) -> Result<(), ClaimError>;
}
pub(super) struct SystemPort;
pub(super) fn check(context: &JobContext) -> Result<(), ClaimError> {
    if context.cancellation_requested()
        || context.deadline().is_none_or(|until| context.now() >= until)
    {
        Err(ClaimError::invalid("entry_context_expired"))
    } else {
        Ok(())
    }
}
impl NativePort for SystemPort {
    fn validate_coordinator(
        &self,
        state: &NativeState,
        handle: &Handle,
        inode: [u8; 24],
        context: &JobContext,
    ) -> Result<(), ClaimError> {
        check(context)?;
        let namespace = state
            .namespace
            .as_ref()
            .ok_or_else(|| ClaimError::invalid("entry_namespace_missing"))?;
        if paths::file_identity(
            handle,
            &mut crate::protected_storage::native_phase::NativePhase::ordinary(&(handle).budget())?,
        )? != inode
        {
            return Err(ClaimError::invalid("entry_coordinator_exclusion_binding"));
        }
        check(context)?;
        paths::validate_regular(
            handle,
            Some(0),
            &mut crate::protected_storage::native_phase::NativePhase::ordinary(&(handle).budget())?,
        )?;
        check(context)?;
        security::verify(
            handle,
            &namespace.account_sid,
            &mut crate::protected_storage::native_phase::NativePhase::ordinary(&(handle).budget())?,
        )?;
        check(context)
    }

    fn validate_election(
        &self,
        state: &NativeState,
        proof: &super::election::RetainedElectionProof,
        context: &JobContext,
    ) -> Result<(), ClaimError> {
        check(context)?;
        let held =
            state.election.as_ref().ok_or_else(|| ClaimError::invalid("entry_retired_election"))?;
        if !held.is_locked()
            || paths::file_identity(
                &held.handle,
                &mut crate::protected_storage::native_phase::NativePhase::ordinary(
                    &held.handle.budget(),
                )?,
            )? != proof.entry_inode
        {
            return Err(ClaimError::invalid("entry_changed_native_election"));
        }
        paths::validate_regular(
            &held.handle,
            Some(0),
            &mut crate::protected_storage::native_phase::NativePhase::ordinary(
                &held.handle.budget(),
            )?,
        )?;
        security::verify(
            &held.handle,
            &proof.account_sid,
            &mut crate::protected_storage::native_phase::NativePhase::ordinary(
                &held.handle.budget(),
            )?,
        )?;
        check(context)
    }
    fn discover(
        &self,
        state: &mut NativeState,
        installation: &Path,
        executable: &Path,
        diagnostics: &super::DiagnosticStorageLocation,
        context: &JobContext,
    ) -> Result<Discovered, ClaimError> {
        check(context)?;
        if state.borrowed || !state.handles.is_empty() || state.namespace.is_some() {
            return Err(ClaimError::invalid("entry_discovery_repeated"));
        }
        let memory = state
            .memory
            .as_ref()
            .ok_or_else(|| ClaimError::invalid("entry_missing_preclaim"))?
            .clone();
        state
            .handles
            .try_reserve_exact(520)
            .map_err(|e| ClaimError::new("entry_handle_reservation", e))?;
        let budget = Budget::Protected(memory);
        let mut resolved =
            account::resolve_namespace(installation, &mut state.handles, budget.clone())?;
        check(context)?;
        let installed = paths::open_retained(
            installation,
            true,
            true,
            budget.clone(),
            &mut state.handles,
            &mut crate::protected_storage::native_phase::NativePhase::ordinary(&(budget.clone()))?,
        )?;
        let installed_path = paths::final_path(
            &state.handles[installed],
            &mut crate::protected_storage::native_phase::NativePhase::ordinary(
                &state.handles[installed].budget(),
            )?,
            crate::protected_storage::native_name::NativeNameDestination::Ordinary,
        )
        .and_then(crate::protected_storage::native_name::NativeName::into_ordinary)?;
        if paths::file_identity(
            &state.handles[installed],
            &mut crate::protected_storage::native_phase::NativePhase::ordinary(
                &state.handles[installed].budget(),
            )?,
        )? != resolved.installation_identity
        {
            return Err(ClaimError::invalid("entry_changed_installation"));
        }
        let expected = paths::open_retained(
            executable,
            false,
            true,
            budget.clone(),
            &mut state.handles,
            &mut crate::protected_storage::native_phase::NativePhase::ordinary(&(budget.clone()))?,
        )?;
        let expected_path = paths::final_path(
            &state.handles[expected],
            &mut crate::protected_storage::native_phase::NativePhase::ordinary(
                &state.handles[expected].budget(),
            )?,
            crate::protected_storage::native_name::NativeNameDestination::Ordinary,
        )
        .and_then(crate::protected_storage::native_name::NativeName::into_ordinary)?;
        if !expected_path.starts_with(&installed_path) {
            return Err(ClaimError::invalid("entry_image_outside_installation"));
        }
        let image = paths::file_identity(
            &state.handles[expected],
            &mut crate::protected_storage::native_phase::NativePhase::ordinary(
                &state.handles[expected].budget(),
            )?,
        )?;
        paths::pin_ancestors(
            &expected_path,
            &mut state.handles,
            budget.clone(),
            &mut crate::protected_storage::native_phase::NativePhase::ordinary(&(budget.clone()))?,
        )?;
        paths::verify_pinned(
            &expected_path,
            image,
            &mut state.handles,
            false,
            budget.clone(),
            &mut crate::protected_storage::native_phase::NativePhase::ordinary(&(budget.clone()))?,
        )?;
        // The actual running host independently binds the expected installed image.
        let current =
            std::env::current_exe().map_err(|e| ClaimError::new("entry_current_image", e))?;
        let actual = paths::open_retained(
            &current,
            false,
            true,
            budget.clone(),
            &mut state.handles,
            &mut crate::protected_storage::native_phase::NativePhase::ordinary(&(budget.clone()))?,
        )?;
        if paths::file_identity(
            &state.handles[actual],
            &mut crate::protected_storage::native_phase::NativePhase::ordinary(
                &state.handles[actual].budget(),
            )?,
        )? != image
        {
            return Err(ClaimError::invalid("entry_running_image_binding"));
        }
        check(context)?;
        account::resolve_diagnostic_root(
            &mut resolved,
            diagnostics,
            &mut state.handles,
            budget,
            context,
        )?;
        let facts = EntryNativeIdentityFacts::new(
            resolved.namespace,
            &resolved.account_sid,
            resolved.session_id,
            image,
        )
        .map_err(|e| ClaimError::new("entry_native_identity", e))?;
        let discovered =
            Discovered { facts, installation_identity: resolved.installation_identity };
        state.namespace = Some(resolved);
        state.image_identity = Some(image);
        Ok(discovered)
    }
    fn elect(&self, state: &mut NativeState, context: &JobContext) -> Result<Elected, ClaimError> {
        election::elect(state, context)
    }
    fn close(&self, state: &mut NativeState, context: &JobContext) -> Result<(), ClaimError> {
        if state.borrowed {
            return Err(ClaimError::invalid("entry_namespace_borrowed"));
        }
        let until =
            context.deadline().ok_or_else(|| ClaimError::invalid("entry_close_deadline"))?;
        if state.close_deadline.is_some_and(|old| until > old) {
            return Err(ClaimError::invalid("entry_close_renewed"));
        }
        state.close_deadline = Some(state.close_deadline.map_or(until, |old| old.min(until)));
        check(context)?;
        if let Some(held) = &mut state.recovery_coordinator {
            check(context)?;
            held.close()?;
        }
        state.recovery_coordinator = None;
        if let Some(held) = &mut state.election {
            check(context)?;
            held.close()?;
        }
        state.election = None;
        check(context)?;
        for handle in state.handles.iter_mut().rev() {
            check(context)?;
            handle.close()?;
        }
        state.handles.clear();
        state.namespace = None;
        state.image_identity = None;
        state.entry_inode = None;
        state.record_origin = None;
        state.memory = None;
        state.retired = true;
        Ok(())
    }
}
pub(super) fn classify(
    read: RecordRead<128>,
    namespace: sugarsubstitute_contracts::lifecycle::NamespaceId,
    inode: Option<[u8; 24]>,
) -> Result<Elected, ClaimError> {
    Ok(match read {
        RecordRead::Missing if inode.is_some() => Elected::Fresh {
            origin: FreshRecordOrigin::Missing,
            inode: inode.ok_or_else(|| ClaimError::invalid("entry_missing_inode"))?,
        },
        RecordRead::Empty if inode.is_some() => Elected::Fresh {
            origin: FreshRecordOrigin::Empty,
            inode: inode.ok_or_else(|| ClaimError::invalid("entry_missing_inode"))?,
        },
        RecordRead::Missing | RecordRead::Empty => Elected::Contended,
        RecordRead::Present(bytes) => match Record::decode(&bytes) {
            Ok(record)
                if record.resource == 0
                    && record.owner.namespace == namespace
                    && record.state == RecordState::Held =>
            {
                Elected::Candidate(record.owner.epoch)
            }
            Ok(record)
                if record.resource == 0
                    && record.owner.namespace == namespace
                    && record.state == RecordState::Released
                    && inode.is_some() =>
            {
                Elected::OrdinaryReleased {
                    record: bytes,
                    inode: inode.ok_or_else(|| ClaimError::invalid("entry_missing_inode"))?,
                }
            }
            Ok(record)
                if record.resource == 0
                    && record.owner.namespace == namespace
                    && record.state == RecordState::Released =>
            {
                Elected::Contended
            }
            _ => Elected::Degraded(EntryOwnerDegradation::RecordUnknown),
        },
    })
}

pub(super) fn is_contention(cause: &ClaimError) -> bool {
    cause.operation() == "claim_exclusion"
        && cause
            .source()
            .and_then(|cause| cause.downcast_ref::<std::fs::TryLockError>())
            .is_some_and(|cause| matches!(cause, std::fs::TryLockError::WouldBlock))
}
