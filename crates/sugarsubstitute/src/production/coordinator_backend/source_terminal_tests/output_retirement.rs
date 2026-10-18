// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retire real native output custody while the settled backend history remains retained.
use super::*;
use crate::production::coordinator_backend::output_fixture;
use sugarsubstitute_contracts::{execution::JobId, transcript::OutputStream};
use sugarsubstitute_execution::windows_output::{OutputIdentity, boundary_fixture::OutputFixture};

/// Prepare the external source with a separate genuinely joined execution lifetime.
fn prepare_external_source(external: &OutputFixture) -> Result<()> {
    let wake = Arc::new(Wake::default());
    let execution = ExecutionOwner::new(Arc::new(HeldClock::default()), wake.clone())?;
    let client = execution.client();
    let native = client.create_native_scope(
        ScopeSpec { id: sugarsubstitute_contracts::execution::ScopeId::new(1)?, deadline: None },
        None,
    )?;
    let key = ResourceKey {
        owner: backend_tests::key()?,
        id: ResourceId::new(1)?,
        generation: ResourceGeneration::new(1)?,
    };
    let mut first = external.reserve(
        &client,
        &native,
        JobSpec {
            id: JobId::new(1)?,
            operation: Operation::NativeLifecycle,
            class: JobClass::NativeObservation,
            deadline: None,
            replacement: None,
            reservation_bytes: MIN_WINDOWS_OUTPUT_BYTES,
        },
        OutputIdentity::Managed(key),
    )?;
    external.end(OutputStream::Stdout);
    external.end(OutputStream::Stderr);
    wake.until(&client, || {
        matches!(
            first.completion.status().phase,
            sugarsubstitute_contracts::execution::JobPhase::Terminal(_)
        )
    })?;
    let completion = first.completion.try_take_completion().ok_or("Original output absent")?;
    assert!(matches!(completion.outcome, JobOutcome::Success(_)));
    drop((completion, first));
    drop(execution);
    assert_eq!(native.snapshot().reserved_bytes, 0);
    Ok(())
}

/// Exercise genuine output completion and policy cleanup with only the pipe boundary faked.
fn exercise(failed: bool) -> Result<()> {
    let external = OutputFixture::new();
    if failed {
        // The same external source now produces its documented preparation error.
        prepare_external_source(&external)?;
    }
    let wake = Arc::new(Wake::default());
    let execution = ExecutionOwner::new(Arc::new(HeldClock::default()), wake.clone())?;
    let client = execution.client();
    let native = client.create_native_scope(
        ScopeSpec { id: sugarsubstitute_contracts::execution::ScopeId::new(1)?, deadline: None },
        None,
    )?;
    let (mut held, leases) = output_fixture::shared(&client, &native, &external)?;
    let backend = held.backend.as_mut();
    let key = leases[0].resource;
    if !failed {
        external.push(OutputStream::Stdout, b"original backend output")?;
        external.end(OutputStream::Stdout);
        external.end(OutputStream::Stderr);
    }
    wake.until(&client, || {
        backend.record(key).ok().and_then(|record| record.output.as_ref()).is_some_and(|output| {
            matches!(
                output.completion.status().phase,
                sugarsubstitute_contracts::execution::JobPhase::Terminal(_)
            )
        })
    })?;
    let loan = if failed {
        None
    } else {
        Some(
            backend
                .record(key)?
                .output
                .as_ref()
                .ok_or("Actual output absent")?
                .control
                .take_batch()?
                .ok_or("Actual output loan absent")?,
        )
    };
    for lease in leases {
        output_fixture::disconnect(backend, lease)?;
    }
    let cleanup = client.create_scope(
        ScopeSpec { id: sugarsubstitute_contracts::execution::ScopeId::new(3)?, deadline: None },
        None,
    )?;
    let jobs = JobSequence::new();
    let scopes = ScopeSequence::new();
    let mut authority = CoordinatorAuthority::new(
        sugarsubstitute_contracts::coordinator_entry::EntrySetupBudget::new(client.now()?)?,
    )?;
    let mut resources = BackendResources {
        client: &client,
        native: &native,
        cleanup: &cleanup,
        jobs: &jobs,
        scopes: &scopes,
        authority: &mut authority,
        capture: None,
    };
    backend.begin_cleanup(key, Some(ShutdownBudget::new(client.now()?)?), &mut resources)?;
    let batch = client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
    backend.consume_existing(0, &batch, &resources, Ok(true))?;
    let original = match &backend
        .record(key)?
        .output_terminal
        .as_ref()
        .ok_or("Original output terminal absent")?
        .outcome
    {
        JobOutcome::Failure(cause) if failed => {
            assert_eq!(cause.stage(), "fixture_output_already_prepared");
            Some(std::ptr::from_ref(cause.source().ok_or("Original native cause absent")?))
        }
        JobOutcome::Success(report) if !failed => {
            assert!(report.retired);
            None
        }
        _ => return Err("Actual output terminal changed".into()),
    };
    backend.cleanup(0, &mut resources)?;
    if !failed {
        assert!(!backend.record(key)?.settled);
        assert!(backend.record(key)?.output.is_some());
        assert!(native.snapshot().reserved_bytes >= MIN_WINDOWS_OUTPUT_BYTES);
        drop(loan);
        backend.cleanup(0, &mut resources)?;
    }
    assert!(backend.record(key)?.settled);
    assert!(backend.record(key)?.output.is_none());
    if failed {
        let JobOutcome::Failure(cause) = &backend
            .record(key)?
            .output_terminal
            .as_ref()
            .ok_or("Failed output custody lost")?
            .outcome
        else {
            return Err("Failed output result replaced".into());
        };
        assert!(std::ptr::eq(
            std::ptr::from_ref(cause.source().ok_or("Failed source lost")?),
            original.ok_or("Original failure pointer absent")?,
        ));
    } else {
        assert!(backend.record(key)?.output_terminal.is_none());
    }
    drop(execution);
    assert_eq!(cleanup.snapshot().reserved_bytes, 0);
    assert_eq!(native.snapshot().reserved_bytes, if failed { MIN_WINDOWS_OUTPUT_BYTES } else { 0 });
    assert!(backend.record(key)?.settled);
    drop(held);
    assert_eq!(native.snapshot().reserved_bytes, 0);
    Ok(())
}

/// Native terminal delivery cannot release a still-held original output loan.
#[test]
fn settled_backend_releases_successful_output_only_after_actual_loan_retirement() -> Result<()> {
    exercise(false)
}

/// Successful native closure leaves the unchanged failed completion and its credit owned.
#[test]
fn settled_backend_keeps_genuine_output_failure_cause_and_credit() -> Result<()> {
    exercise(true)
}
