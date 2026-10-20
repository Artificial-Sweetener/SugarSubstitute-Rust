// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise real source closure and fixed executor credit under bounded pressure.
use super::*;
use crate::production::parent_lifecycle::profile_observation::tests::Fixture;
use sugarsubstitute_contracts::execution::{AdmissionError, ExecutionPort, JobId};

type TestResult<T = ()> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

fn spec(id: u64, bytes: u64) -> TestResult<JobSpec> {
    Ok(JobSpec {
        id: JobId::new(id)?,
        operation: Operation::Readiness,
        class: JobClass::InfrastructureLifecycle,
        deadline: Some(Moment::from_nanos(30_000_000_000)),
        reservation_bytes: bytes,
        replacement: None,
    })
}

#[test]
fn successful_original_source_close_returns_credit_before_http_cleanup_admission() -> TestResult {
    let mut fixture = Fixture::new()?;
    let pressure = fixture.memory_bytes - 2 * FRONTEND_SOURCE_BYTES - 8 * 1024 * 1024;
    let mut pressure_job =
        fixture.client.submit_blocking(&fixture.scope, spec(1, pressure)?, move |context| {
            context.memory().claim(pressure)
        })?;
    fixture.terminal(pressure_job.spec().id)?;
    let pressure_terminal =
        pressure_job.try_take_completion().ok_or("Pressure completion absent")?;
    assert!(matches!(pressure_terminal.outcome, JobOutcome::Success(_)));
    let mut source = sugarsubstitute_supervision::installed_frontend::boundary_fixture::owner()?;
    source.submit_inspection(&fixture.client, &fixture.scope, spec(2, FRONTEND_SOURCE_BYTES)?)?;
    fixture.terminal(JobId::new(2)?)?;
    let lease = source.take_inspection()?.ok_or("Actual inspection absent")?;
    let mut close = SourceClose::default();
    close.submit(&source, &fixture.client, &fixture.scope, spec(3, FRONTEND_SOURCE_BYTES)?)?;
    assert!(close.awaiting_source());
    let rejected =
        fixture.client.submit_blocking(&fixture.scope, spec(4, READINESS_TRANSPORT_BYTES)?, |_| {
            Ok::<_, std::io::Error>(())
        });
    assert!(matches!(rejected, Err(AdmissionError::MemorySaturated { .. })));
    fixture.terminal(JobId::new(3)?)?;
    close.consume()?;
    assert!(close.closed() && !close.awaiting_source());
    assert!(lease.validate_live().is_err());
    fixture.reservation_at_most(pressure + FRONTEND_SOURCE_BYTES)?;
    let mut http_close = fixture.client.submit_blocking(
        &fixture.scope,
        spec(5, READINESS_TRANSPORT_BYTES)?,
        |_| Ok::<_, std::io::Error>(()),
    )?;
    fixture.terminal(http_close.spec().id)?;
    let http_terminal = http_close.try_take_completion().ok_or("HTTP cleanup completion absent")?;
    assert!(matches!(http_terminal.outcome, JobOutcome::Success(())));
    drop(http_terminal);
    drop(http_close);
    drop(lease);
    drop(source);
    drop(close);
    drop(pressure_terminal);
    drop(pressure_job);
    fixture.finish()
}

#[test]
fn foreign_source_completion_retains_credit_and_cannot_admit_a_second_close() -> TestResult {
    let mut fixture = Fixture::new()?;
    let mut source = sugarsubstitute_supervision::installed_frontend::boundary_fixture::owner()?;
    source.submit_inspection(&fixture.client, &fixture.scope, spec(1, FRONTEND_SOURCE_BYTES)?)?;
    fixture.terminal(JobId::new(1)?)?;
    let lease = source.take_inspection()?.ok_or("Actual inspection absent")?;
    let mut close = SourceClose::default();
    close.submit(&source, &fixture.client, &fixture.scope, spec(2, FRONTEND_SOURCE_BYTES)?)?;
    fixture.terminal(JobId::new(2)?)?;
    // Corrupt only external completion identity on an actually admitted terminal.
    let job = close.job.as_mut().ok_or("Original close absent")?;
    let mut terminal = job.try_take_completion().ok_or("Original close terminal absent")?;
    terminal.spec.id = JobId::new(99)?;
    assert!(close.accept_terminal(terminal).is_err());
    assert!(close.consume().is_err());
    assert!(!close.closed() && !close.awaiting_source());
    close.submit(&source, &fixture.client, &fixture.scope, spec(3, FRONTEND_SOURCE_BYTES)?)?;
    assert!(close.job.is_none() && close.terminal.is_some());
    drop(close);
    drop(lease);
    drop(source);
    fixture.finish()
}
