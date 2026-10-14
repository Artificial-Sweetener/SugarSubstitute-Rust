// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prove real central export admission, original control checks and retained native custody.
use super::{test_support::*, *};
use crate::{runtime_test_support::TestResult, scope::lock};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use sugarsubstitute_contracts::execution::*;

#[test]
fn original_128_kib_and_68_outputs_issue_once_and_credit_survives_last_output() -> TestResult {
    let mut pair = Pair::new()?;
    let origin = pair.rig.client.report_export_admission(&pair.context)?.into_origin();
    assert_eq!(origin.retained_bytes(), REPORT_EXPORT_BYTES);
    assert!(origin.control_storage_bytes() <= 4096);
    assert!(pair.rig.client.report_export_admission(&pair.context).is_err());
    assert!(pair.context.memory().claim(1).is_err());
    let mut outputs = Vec::new();
    for _ in 0..REPORT_EXPORT_OUTPUTS {
        outputs.push(origin.reserve_output()?);
    }
    assert!(matches!(origin.reserve_output(), Err(AdmissionError::ReportExportOutputCapacity)));
    let snapshot = pair.rig.scope.snapshot();
    assert_eq!(snapshot.native_outputs_in_custody, 68);
    assert_eq!(snapshot.native_reservations, 1);
    assert_eq!(Arc::weak_count(&origin.state()?.counters), 1);
    let alias = origin.clone();
    assert!(alias.same_origin(&origin));
    drop(origin);
    drop(alias);
    assert!(pair.context.memory().claim(1).is_err());
    drop(outputs);
    let credit = pair.context.memory().claim(REPORT_EXPORT_BYTES)?;
    assert_eq!(pair.rig.scope.snapshot().native_reservations, 0);
    drop(credit);
    assert!(pair.rig.client.report_export_admission(&pair.context).is_err());
    let completion = pair.finish_main()?;
    assert!(matches!(completion.outcome, JobOutcome::Success(())));
    drop(completion.memory);
    Ok(())
}

#[test]
fn counterfeit_memory_and_native_ledgers_reject_before_original_clock_or_caller_ports() -> TestResult
{
    let pair = Pair::new()?;
    let ports = Arc::new(Ports(AtomicUsize::new(0)));
    let reads = Arc::new(AtomicUsize::new(0));
    let callback = reads.clone();
    pair.rig.clock.on_next_current_thread_observation(Box::new(move || {
        callback.fetch_add(1, Ordering::AcqRel);
    }));
    let fake = supplied(
        &pair.context,
        MemoryLease::new(REPORT_EXPORT_BYTES, Box::new(ForeignRelease)),
        None,
        ports.clone(),
    );
    assert!(pair.rig.client.report_export_admission(&fake).is_err());
    let foreign = JobContext::new(
        pair.context.owner_id(),
        pair.context.scope_id(),
        pair.context.spec(),
        pair.context.deadline(),
        ports.clone(),
        ports.clone(),
        AdmittedJobResources::new(pair.context.memory().clone(), Some(Box::new(ForeignNative))),
    );
    assert!(pair.rig.client.report_export_admission(&foreign).is_err());
    assert_eq!(reads.load(Ordering::Acquire), 0);
    let facts = supplied(&pair.context, pair.context.memory().clone(), None, ports.clone());
    let origin = pair.rig.client.report_export_admission(&facts)?.into_origin();
    assert_eq!(reads.load(Ordering::Acquire), 1);
    assert_eq!(ports.0.load(Ordering::Acquire), 0);
    assert_eq!(origin.original_context()?.now(), Moment::from_nanos(0));
    assert!(!origin.original_context()?.cancellation_requested());
    Ok(())
}

#[test]
fn wrong_actual_counter_allocation_never_matches_numeric_context() -> TestResult {
    let pair = Pair::new()?;
    let origin = pair.rig.client.report_export_admission(&pair.context)?.into_origin();
    let fake = Arc::new(resources::Counters::new());
    let core = pair.rig.client.core_for_test();
    let mut state = lock(&core.state);
    let resource = state
        .report_resources
        .get_mut(&pair.context.job_id())
        .ok_or("Actual export ledger absent")?;
    let saved = std::mem::replace(&mut resource.observation, Arc::downgrade(&fake));
    drop(state);
    assert!(origin.check_effect().is_err());
    assert!(origin.reserve_output().is_err());
    let mut state = lock(&core.state);
    let resource = state
        .report_resources
        .get_mut(&pair.context.job_id())
        .ok_or("Actual export ledger absent")?;
    resource.observation = saved;
    drop(state);
    origin.check_effect()?;
    Ok(())
}

#[test]
fn actual_clock_callback_closing_main_vetoes_effect_and_output() -> TestResult {
    let pair = Pair::new()?;
    let admission = pair.rig.client.report_export_admission(&pair.context)?;
    admission.check_effect()?;
    let origin = admission.into_origin();
    let core = pair.rig.client.core_for_test();
    let scope = pair.rig.scope.id();
    pair.rig.clock.on_next_current_thread_observation(Box::new(move || core.close_scope(scope)));
    assert!(matches!(origin.check_effect(), Err(AdmissionError::ScopeClosed { .. })));
    assert!(origin.reserve_output().is_err());
    Ok(())
}

#[test]
fn actual_main_cancellation_and_deadline_veto_fresh_effects() -> TestResult {
    let pair = Pair::new()?;
    let origin = pair.rig.client.report_export_admission(&pair.context)?.into_origin();
    pair.main.cancel();
    assert!(origin.check_effect().is_err());
    drop(origin);
    drop(pair);
    let pair = Pair::new()?;
    let origin = pair.rig.client.report_export_admission(&pair.context)?.into_origin();
    pair.rig.clock.advance(10_000_000_000);
    assert!(matches!(origin.check_effect(), Err(AdmissionError::DeadlineExpired { .. })));
    assert!(origin.reserve_output().is_err());
    Ok(())
}

#[test]
fn cleanup_waits_for_main_completion_and_uses_once_activated_original_cutoff() -> TestResult {
    let mut pair = Pair::new()?;
    let origin = pair.rig.client.report_export_admission(&pair.context)?.into_origin();
    let output = origin.reserve_output()?;
    let permit = pair.permit.as_ref().ok_or("Held cleanup absent")?;
    assert!(
        pair.rig
            .client
            .freeze_report_export_cleanup_deadline(permit, std::time::Duration::from_secs(15))
            .is_err()
    );
    assert!(pair.rig.client.report_export_retirement(&origin, &pair.context).is_err());
    let completion = pair.finish_main()?;
    assert!(matches!(completion.outcome, JobOutcome::Success(())));
    drop(completion.memory);
    assert!(origin.check_effect().is_err());
    let permit = pair.permit.as_ref().ok_or("Held cleanup absent")?;
    assert!(
        pair.rig
            .client
            .freeze_report_export_cleanup_deadline(permit, std::time::Duration::ZERO)
            .is_err()
    );
    assert!(
        pair.rig
            .client
            .freeze_report_export_cleanup_deadline(permit, std::time::Duration::from_secs(16))
            .is_err()
    );
    let cutoff = pair
        .rig
        .client
        .freeze_report_export_cleanup_deadline(permit, std::time::Duration::from_secs(15))?;
    pair.rig.clock.advance(1_000_000_000);
    assert_eq!(
        pair.rig
            .client
            .freeze_report_export_cleanup_deadline(permit, std::time::Duration::from_secs(15))?,
        cutoff
    );
    assert!(
        pair.rig
            .client
            .freeze_report_export_cleanup_deadline(permit, std::time::Duration::from_secs(14))
            .is_err()
    );
    let (mut cleanup, context, actual) = pair.activate()?;
    let retirement = pair.rig.client.report_export_retirement(&origin, &context)?;
    assert_eq!(actual, cutoff);
    assert!(retirement.matches_origin(&origin));
    assert_eq!(retirement.cleanup_cutoff(), cutoff.deadline());
    assert_eq!(retirement.original_context().now(), Moment::from_nanos(1_000_000_000));
    retirement.check()?;
    assert!(pair.context.memory().claim(1).is_err());
    drop(output);
    assert!(pair.context.memory().claim(1).is_err());
    pair.rig.clock.advance(14_000_000_000);
    assert!(matches!(retirement.check(), Err(AdmissionError::DeadlineExpired { .. })));
    assert!(pair.rig.client.report_export_retirement(&origin, &context).is_err());
    drop(origin);
    assert!(pair.context.memory().claim(1).is_err());
    drop(retirement);
    drop(pair.context.memory().claim(REPORT_EXPORT_BYTES)?);
    let completion = pair.finish_cleanup(&mut cleanup)?;
    assert!(matches!(completion.outcome, JobOutcome::Cancelled | JobOutcome::TimedOut));
    drop(completion.memory);
    Ok(())
}

#[test]
fn close_uses_actual_cleanup_ports_and_foreign_origins_reject() -> TestResult {
    let mut pair = Pair::new()?;
    let foreign = Pair::new()?;
    let origin = pair.rig.client.report_export_admission(&pair.context)?.into_origin();
    let other = foreign.rig.client.report_export_admission(&foreign.context)?.into_origin();
    let completion = pair.finish_main()?;
    assert!(matches!(completion.outcome, JobOutcome::Success(())));
    drop(completion.memory);
    let (mut cleanup, context, _) = pair.activate()?;
    assert!(foreign.rig.client.report_export_retirement(&origin, &context).is_err());
    assert!(pair.rig.client.report_export_retirement(&other, &context).is_err());
    let ports = Arc::new(Ports(AtomicUsize::new(0)));
    let facts = supplied(&context, context.memory().clone(), None, ports.clone());
    let retirement = pair.rig.client.report_export_retirement(&origin, &facts)?;
    assert!(!retirement.matches_origin(&other));
    assert_eq!(ports.0.load(Ordering::Acquire), 0);
    pair.cleanup_scope.cancel();
    assert!(retirement.check().is_err());
    assert_eq!(ports.0.load(Ordering::Acquire), 0);
    let completion = pair.finish_cleanup(&mut cleanup)?;
    assert!(matches!(completion.outcome, JobOutcome::Cancelled | JobOutcome::TimedOut));
    drop(completion.memory);
    Ok(())
}

#[test]
fn shutdown_observes_original_native_output_custody_after_main_acknowledgment() -> TestResult {
    let mut pair = Pair::new()?;
    let origin = pair.rig.client.report_export_admission(&pair.context)?.into_origin();
    let output = origin.reserve_output()?;
    let completion = pair.finish_main()?;
    assert!(matches!(completion.outcome, JobOutcome::Success(())));
    drop(completion.memory);
    pair.rig.owner.request_shutdown(Moment::from_nanos(15_000_000_000));
    let progress = pair.rig.owner.poll_shutdown()?;
    assert!(!progress.settled);
    assert!(
        progress
            .native_output_custody
            .iter()
            .any(|row| row.original_job_id == pair.context.job_id() && row.outputs_in_custody == 1)
    );
    assert!(origin.check_effect().is_err());
    drop(origin);
    assert!(pair.context.memory().claim(1).is_err());
    drop(output);
    assert!(pair.rig.owner.poll_shutdown()?.native_output_custody.is_empty());
    Ok(())
}
