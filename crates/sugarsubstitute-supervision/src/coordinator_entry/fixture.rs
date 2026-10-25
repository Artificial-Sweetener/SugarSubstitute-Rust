// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compose crate-private CPU fixtures from actual entry execution and external native facts.
use super::{
    CoordinatorEntryInput, CoordinatorEntryOwner, ENTRY_PREPARATION_BYTES, EntryObservation,
    election::{ElectionRecordOrigin, RetainedElectionProof},
    namespace::NativeState,
    native::{self, Discovered, Elected, NativePort},
};
use crate::{
    ClaimError, durable_storage::protected::account::ResolvedNamespace, native_claims::lock,
};
use std::{
    error::Error,
    num::NonZeroUsize,
    path::PathBuf,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
use sugarsubstitute_contracts::{
    configuration::{
        ConnectionConfiguration, LoopbackOrigin, Presentation, ProfileId, StartupConfiguration,
        Theme,
    },
    coordinator_entry::{EntryLaunchRequest, EntryNativeIdentityFacts, EntrySetupBudget},
    execution::{
        Clock, CompletionWake, ExecutionError, ExecutionPort, JobClass, JobId, JobSpec, Moment,
        Operation, OwnedJob, Scope, ScopeId, ScopeSpec, WaitFuture,
    },
    lifecycle::NamespaceId,
};
use sugarsubstitute_execution::{ExecutionClient, ExecutionOwner};
type TestResult = Result<(), Box<dyn Error>>;
mod boundary;
mod harness;
pub(crate) use boundary::{ClockPort, Fake, Gate, Stage, Wake, facts, resolved};
pub(crate) use harness::Harness;
/// Adopt a fixture boundary's real charged file into the retained namespace slot.
pub(crate) fn install_recovery(
    proof: &RetainedElectionProof,
    file: &mut Option<crate::durable_storage::protected::exclusion::HeldFile<128>>,
) -> Result<(), ClaimError> {
    let mut state = lock(&proof.lifetime.state);
    if !matches!(proof.origin, ElectionRecordOrigin::RecoveredHeld { .. })
        || state.recovery_coordinator.is_some()
        || file.is_none()
    {
        return Err(ClaimError::invalid("recovery_fixture_state"));
    }
    state.recovery_coordinator = file.take();
    Ok(())
}
/// Observe this actual mutex's WouldBlock result while an initializer borrows it.
pub(crate) fn initialization_guard_held(proof: &RetainedElectionProof) -> bool {
    matches!(proof.lifetime.state.try_lock(), Err(std::sync::TryLockError::WouldBlock))
}
