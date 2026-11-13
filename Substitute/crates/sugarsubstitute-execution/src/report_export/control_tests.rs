// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject original control failures against actual Core records and boundaries.
use super::test_support::*;
use crate::{runtime_test_support::TestResult, scope::lock};
use sugarsubstitute_contracts::execution::*;

#[test]
fn ordinary_matching_spec_unpaired_foreign_and_retired_main_never_issue_export() -> TestResult {
    let mut pair = Pair::new()?;
    let foreign = Pair::new()?;
    assert!(foreign.rig.client.report_export_admission(&pair.context).is_err());
    let core = pair.rig.client.core_for_test();
    {
        let mut state = lock(&core.state);
        state.jobs.get_mut(&pair.context.job_id()).ok_or("Main absent")?.report_export = false;
    }
    assert!(pair.rig.client.report_export_admission(&pair.context).is_err());
    {
        let mut state = lock(&core.state);
        state.jobs.get_mut(&pair.context.job_id()).ok_or("Main absent")?.report_export = true;
        for record in state.jobs.values_mut() {
            if let Some(binding) = &mut record.report_cleanup {
                binding.paired = false;
            }
        }
    }
    assert!(pair.rig.client.report_export_admission(&pair.context).is_err());
    {
        let mut state = lock(&core.state);
        for record in state.jobs.values_mut() {
            if let Some(binding) = &mut record.report_cleanup {
                binding.paired = true;
            }
        }
    }
    let origin = pair.rig.client.report_export_admission(&pair.context)?.into_origin();
    let completion = pair.finish_main()?;
    assert!(matches!(completion.outcome, JobOutcome::Success(())));
    drop(completion.memory);
    assert!(pair.rig.client.report_export_admission(&pair.context).is_err());
    assert!(origin.check_effect().is_err());
    Ok(())
}

#[test]
fn changed_original_scope_cutoff_is_checked_on_both_main_and_cleanup() -> TestResult {
    let mut pair = Pair::new()?;
    let origin = pair.rig.client.report_export_admission(&pair.context)?.into_origin();
    let core = pair.rig.client.core_for_test();
    {
        let mut state = lock(&core.state);
        state.scopes.get_mut(&pair.rig.scope.id()).ok_or("Original scope absent")?.deadline =
            Some(Moment::from_nanos(0));
    }
    assert!(matches!(origin.check_effect(), Err(AdmissionError::DeadlineExpired { .. })));
    {
        let mut state = lock(&core.state);
        state.scopes.get_mut(&pair.rig.scope.id()).ok_or("Original scope absent")?.deadline = None;
    }
    let completion = pair.finish_main()?;
    assert!(matches!(completion.outcome, JobOutcome::Success(())));
    drop(completion.memory);
    let (mut cleanup, context, _) = pair.activate()?;
    let retirement = pair.rig.client.report_export_retirement(&origin, &context)?;
    let scope = pair.cleanup_scope.id();
    pair.rig.clock.on_next_current_thread_observation(Box::new(move || {
        if let Some(record) = lock(&core.state).scopes.get_mut(&scope) {
            record.deadline = Some(Moment::from_nanos(0));
        }
    }));
    assert!(matches!(retirement.check(), Err(AdmissionError::DeadlineExpired { .. })));
    let completion = pair.finish_cleanup(&mut cleanup)?;
    assert!(matches!(completion.outcome, JobOutcome::Success(()) | JobOutcome::TimedOut));
    drop(completion.memory);
    assert!(retirement.check().is_err());
    Ok(())
}

#[test]
fn actual_original_clock_failure_preserves_cause_without_invoking_supplied_ports() -> TestResult {
    let pair = Pair::new()?;
    let origin = pair.rig.client.report_export_admission(&pair.context)?.into_origin();
    pair.rig.clock.on_next_current_thread_observation(Box::new(|| {
        std::panic::resume_unwind(Box::new(String::from("report export original clock failure")));
    }));
    let cause = origin.check_effect().err().ok_or("Original clock failure accepted")?;
    assert!(cause.to_string().contains("report export original clock failure"));
    assert!(origin.reserve_output().is_err());
    Ok(())
}
