// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compose crate-private CPU fixtures from actual entry execution and external native facts.
use super::{
    CoordinatorEntryInput, CoordinatorEntryOwner, ENTRY_PREPARATION_BYTES, EntryObservation,
    election::{ElectionRecordOrigin, RetainedElectionProof},
    namespace::NativeState,
    native::{self, Discovered, Elected, NativePort},
};
use crate::{ClaimError, native_claims::lock, protected_storage::account::ResolvedNamespace};
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
