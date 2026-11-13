// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise preparation sequencing and retained failure custody against the external content port.

use super::*;
use crate::startup_runtime::test_content::*;
use std::error::Error;
#[test]
fn waiting_preserves_one_start_and_actual_ready_admits_only_the_same_native_transition()
-> TestResult {
    let mut content = Content::new()?;
    let mut state = PreparationState::new();
    content.steps.extend([Ok(StartupPreparationStep::Waiting), Ok(StartupPreparationStep::Ready)]);
    assert!(state.start(&mut content, Arc::new(Wake)));
    state.activity(&mut content, budget()?);
    assert_eq!(state.exit(), None);
    state.activity(&mut content, budget()?);
    assert_eq!(state.exit(), Some(PreparationExit::Ready));
    state.activity(&mut content, budget()?);
    assert_eq!(content.calls, ["start", "prepare", "prepare"]);
    assert_eq!(content.budgets, [32, 32]);
    assert_eq!(state.native_return(&mut content, 0), Some(PreparationExit::Ready));
    assert!(state.take_failure().is_none());
    Ok(())
}

#[test]
fn failed_start_keeps_original_cause_and_drains_actual_shutdown_after_late_ready() -> TestResult {
    let mut content = Content::new()?;
    content.start_failure = true;
    content.close_failure = true;
    content.steps.extend([
        Ok(StartupPreparationStep::Ready),
        Ok(StartupPreparationStep::Waiting),
        Ok(StartupPreparationStep::Ready),
    ]);
    content.settlements.extend([
        Ok(false),
        Err(std::io::Error::other("later poll cause").into()),
        Ok(true),
    ]);
    let mut state = PreparationState::new();
    assert!(!state.start(&mut content, Arc::new(Wake)));
    state.close(&mut content);
    for _ in 0..2 {
        state.activity(&mut content, budget()?);
        assert_eq!(state.exit(), None);
    }
    state.activity(&mut content, budget()?);
    assert_eq!(state.exit(), Some(PreparationExit::Closed));
    state.activity(&mut content, budget()?);
    assert_eq!(
        content.calls,
        [
            "start",
            "close",
            "native_retired",
            "prepare",
            "settle",
            "prepare",
            "settle",
            "prepare",
            "settle"
        ]
    );
    let cause = state.take_failure().ok_or("Original failure lost")?;
    assert_eq!(cause.source().ok_or("Source lost")?.to_string(), "original authentication cause");
    Ok(())
}

#[test]
fn preparation_error_retains_native_absence_and_never_implies_worker_settlement() -> TestResult {
    let mut content = Content::new()?;
    let mut state = PreparationState::new();
    assert!(state.start(&mut content, Arc::new(Wake)));
    content.steps.push_back(Err(std::io::Error::other("capture attachment cause").into()));
    state.activity(&mut content, budget()?);
    assert!(state.closing());
    assert_eq!(state.exit(), None);
    content.settlements.push_back(Ok(true));
    state.activity(&mut content, budget()?);
    assert_eq!(state.exit(), Some(PreparationExit::Closed));
    assert_eq!(content.calls.iter().filter(|call| **call == "native_retired").count(), 1);
    assert!(!content.calls.contains(&"join") && !content.calls.contains(&"frame"));
    assert_eq!(
        state.take_failure().ok_or("Cause lost")?.source().ok_or("Source lost")?.to_string(),
        "capture attachment cause"
    );
    Ok(())
}

#[test]
fn duplicate_start_fences_before_effect_and_closes_the_existing_lifetime() -> TestResult {
    let mut content = Content::new()?;
    let mut state = PreparationState::new();
    assert!(state.start(&mut content, Arc::new(Wake)));
    assert!(!state.start(&mut content, Arc::new(Wake)));
    assert_eq!(content.calls, ["start", "close", "native_retired"]);
    assert_eq!(state.exit(), None);
    Ok(())
}

#[test]
fn early_or_failed_native_return_keeps_work_owned_until_same_close_transaction_settles()
-> TestResult {
    for status in [0, 7] {
        let mut content = Content::new()?;
        let mut state = PreparationState::new();
        assert!(state.start(&mut content, Arc::new(Wake)));
        assert_eq!(state.native_return(&mut content, status), None);
        assert_eq!(state.native_return(&mut content, status), None);
        content.steps.push_back(Ok(StartupPreparationStep::Ready));
        content.settlements.push_back(Ok(true));
        state.activity(&mut content, budget()?);
        assert_eq!(state.exit(), Some(PreparationExit::Closed));
        assert_eq!(content.calls.iter().filter(|call| **call == "start").count(), 1);
        assert_eq!(content.calls.iter().filter(|call| **call == "close").count(), 1);
        assert!(!content.calls.contains(&"join"));
        assert!(
            state
                .take_failure()
                .ok_or("Native cause lost")?
                .to_string()
                .contains(&format!("status {status}"))
        );
    }
    Ok(())
}

#[test]
fn failed_sta_boundary_resumes_started_owner_without_second_start_or_ready_admission() -> TestResult
{
    let mut content = Content::new()?;
    let mut state = PreparationState::closing_started(
        &mut content,
        DesktopError::message("ui_apartment_acquire", "native cause"),
    );
    content.steps.push_back(Ok(StartupPreparationStep::Ready));
    state.activity(&mut content, budget()?);
    assert_eq!(state.exit(), None);
    content.settlements.push_back(Ok(true));
    state.activity(&mut content, budget()?);
    assert_eq!(state.exit(), Some(PreparationExit::Closed));
    assert!(!content.calls.contains(&"start"));
    assert_eq!(
        state.take_failure().ok_or("STA cause lost")?.to_string(),
        "ui_apartment_acquire: native cause"
    );
    Ok(())
}
