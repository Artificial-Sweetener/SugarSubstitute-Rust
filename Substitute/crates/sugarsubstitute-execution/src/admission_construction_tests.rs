// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Reject construction through genuine admission without publishing partial ownership.

use super::*;
use crate::runtime_test_support::{ClockPort, ManualClock, TestResult, Wake, spec};
use std::{error::Error, io, panic::AssertUnwindSafe};
use sugarsubstitute_contracts::execution::ExecutionPort;

fn fixture() -> TestResult<(Arc<Core>, crate::ExecutionClient, Scope)> {
    let core = Core::new(
        ExecutionId::new(1)?,
        Arc::new(ClockPort(Arc::new(ManualClock::default()))),
        Arc::new(Wake::default()),
        crate::runtime::DEFAULT_MEMORY_BYTES,
    );
    let client = crate::ExecutionClient { core: core.clone() };
    let scope = client.create_scope(
        sugarsubstitute_contracts::execution::ScopeSpec { id: ScopeId::new(1)?, deadline: None },
        None,
    )?;
    Ok((core, client, scope))
}

fn assert_no_admission(core: &Core, scope: &Scope) {
    let state = lock(&core.state);
    assert!(state.jobs.is_empty());
    assert!(state.queue.is_empty());
    assert!(state.retired.is_empty());
    assert!(state.symbol_resources.is_empty());
    assert!(state.latest_job.is_none());
    assert_eq!(state.bytes, 0);
    drop(state);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
}

#[test]
fn failed_original_claim_returns_identical_input_without_consuming_job_identity() -> TestResult {
    let (core, _, scope) = fixture()?;
    let input = Arc::new(17_u64);
    let actual = input.clone();
    let specification = spec(1, JobClass::InfrastructureLifecycle, 16_384)?;
    let result = core.admit_owned(
        &scope,
        specification,
        RequestKind::Blocking,
        input,
        |input, context, _, _| -> Result<(QueuedJob, ()), _> {
            let cause = context.memory().claim(16_385).err().ok_or_else(|| {
                (
                    AdmissionError::Unexpected(crate::runtime::error(io::Error::other(
                        "Oversized genuine claim unexpectedly succeeded",
                    ))),
                    input.clone(),
                )
            })?;
            Err((AdmissionError::Unexpected(crate::runtime::error(cause)), input))
        },
    );
    let (cause, returned) = result.err().ok_or("Failed claim admitted partial ownership")?;
    assert!(Arc::ptr_eq(&returned, &actual));
    let AdmissionError::Unexpected(cause) = cause else {
        return Err("Original claim failure was replaced".into());
    };
    let original = cause
        .source()
        .and_then(|cause| {
            cause.downcast_ref::<sugarsubstitute_contracts::execution::MemoryClaimError>()
        })
        .ok_or("Actual memory claim cause was not retained")?;
    assert_eq!(original.requested_bytes, 16_385);
    assert_eq!(original.available_bytes, 16_384);
    assert_no_admission(&core, &scope);
    Ok(())
}

#[test]
fn rejected_symbol_construction_preserves_unpaired_cleanup_and_no_native_record() -> TestResult {
    let (core, client, scope) = fixture()?;
    let cleanup_scope = client.create_scope(
        sugarsubstitute_contracts::execution::ScopeSpec { id: ScopeId::new(2)?, deadline: None },
        None,
    )?;
    let cleanup_spec =
        spec(1, JobClass::InfrastructureLifecycle, crate::symbol_admission::CLEANUP_BYTES)?;
    let mut main_spec =
        spec(2, JobClass::InfrastructureOrdinary, crate::symbol_admission::MAIN_BYTES)?;
    main_spec.deadline = Some(Moment::from_nanos(10_000_000_000));
    let cleanup = client
        .reserve_symbol_cleanup(&cleanup_scope, cleanup_spec, &scope, main_spec, |_| {
            Ok::<_, io::Error>(())
        })
        .map_err(|rejected| rejected.error)?;
    let input = Arc::new(23_u64);
    let actual = input.clone();
    let rejected = core
        .admit_owned(
            &scope,
            main_spec,
            RequestKind::SymbolHelper { cleanup: cleanup_spec.id },
            input,
            |input, context, _, _| -> Result<(QueuedJob, ()), _> {
                let cause = context
                    .memory()
                    .claim(crate::symbol_admission::MAIN_BYTES)
                    .err()
                    .ok_or_else(|| (AdmissionError::InvalidSymbolAdmission, input.clone()))?;
                Err((AdmissionError::Unexpected(crate::runtime::error(cause)), input))
            },
        )
        .err()
        .ok_or("Rejected symbol builder published a main owner")?;
    assert!(Arc::ptr_eq(&rejected.1, &actual));
    assert!(matches!(rejected.0, AdmissionError::Unexpected(_)));
    let state = lock(&core.state);
    assert_eq!(state.latest_job, Some(cleanup_spec.id));
    assert_eq!(state.jobs.len(), 1);
    assert_eq!(state.queue.len(), 1);
    assert!(state.symbol_resources.is_empty());
    assert!(
        !state
            .jobs
            .get(&cleanup_spec.id)
            .and_then(|record| record.symbol_cleanup.as_ref())
            .ok_or("Actual held cleanup disappeared")?
            .paired
    );
    drop(state);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    assert_eq!(cleanup_scope.snapshot().reserved_bytes, crate::symbol_admission::CLEANUP_BYTES);
    drop(cleanup);
    core.flush_notifications();
    while let Some(queued) = core.take_stopped_cpu(|_| true) {
        crate::task::stop_queued(queued, crate::task::Stop::Cancelled);
    }
    while let Some(retired) = core.take_retired_cpu(|_| true) {
        crate::task::retire(retired);
    }
    assert!(core.settled());
    assert_eq!(cleanup_scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn constructor_unwind_releases_actual_claim_and_context_after_core_unlock() -> TestResult {
    let (core, _, scope) = fixture()?;
    let specification = spec(1, JobClass::InfrastructureLifecycle, 16_384)?;
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        core.admit_owned(
            &scope,
            specification,
            RequestKind::Blocking,
            (),
            |(), context, _, _| -> Result<(QueuedJob, ()), (AdmissionError, ())> {
                let _claim = context.memory().claim(16_384).map_err(|cause| {
                    (AdmissionError::Unexpected(crate::runtime::error(cause)), ())
                })?;
                // No Job, OwnedJob, Control, or external constructor exists at this boundary.
                std::panic::resume_unwind(Box::new("Actual constructor failure"));
            },
        )
    }));
    assert!(result.is_err());
    assert_no_admission(&core, &scope);
    Ok(())
}
