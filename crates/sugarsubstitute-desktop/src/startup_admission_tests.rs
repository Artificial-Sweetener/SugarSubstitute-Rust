// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify causal native construction against actual pre-Tao preparation and cleanup.
use super::*;
use crate::{StartupPreparationStep, startup_runtime::test_content::*};
use std::{cell::Cell, rc::Rc};
#[test]
fn actual_preparation_wake_and_ready_precede_native_construction_without_surface_work() -> TestResult
{
    let bootstrap = Bootstrap::new()?;
    let mut content = Content::new()?;
    content.steps.extend([Ok(StartupPreparationStep::Waiting), Ok(StartupPreparationStep::Ready)]);
    let ready = content.ready.clone();
    let ((), lifetime) = admit_with(&bootstrap, &mut content, budget()?, None, None, || {
        assert!(ready.get());
        Ok(((), WakeLifetime::new(), Arc::new(Wake)))
    })?;
    assert_eq!(content.calls, ["start", "prepare", "prepare"]);
    assert_eq!(content.budgets, [32, 32]);
    bootstrap.disconnect()?;
    lifetime.disconnect();
    Ok(())
}
#[test]
fn expired_original_limit_closes_before_native_construction_and_ignores_late_ready() -> TestResult {
    let bootstrap = Bootstrap::new()?;
    let mut content = Content::new()?;
    content.steps.push_back(Ok(StartupPreparationStep::Ready));
    content.settlements.push_back(Ok(true));
    let constructed = Rc::new(Cell::new(false));
    let marker = constructed.clone();
    let result =
        admit_with(&bootstrap, &mut content, budget()?, Some(Instant::now()), None, || {
            marker.set(true);
            Ok(((), WakeLifetime::new(), Arc::new(Wake)))
        });
    assert!(result.is_err());
    assert!(!constructed.get());
    assert_eq!(content.calls, ["start", "close", "native_retired", "prepare", "settle"]);
    assert!(bootstrap.wake().wake().is_err());
    Ok(())
}
#[test]
fn failed_native_construction_retains_original_owner_and_cause_until_actual_settlement()
-> TestResult {
    let bootstrap = Bootstrap::new()?;
    let mut content = Content::new()?;
    content.steps.extend([
        Ok(StartupPreparationStep::Ready),
        Ok(StartupPreparationStep::Waiting),
        Ok(StartupPreparationStep::Ready),
    ]);
    content.settlements.extend([Ok(false), Ok(true)]);
    let result: Result<((), WakeLifetime), _> =
        admit_with(&bootstrap, &mut content, budget()?, None, None, || {
            Err(DesktopError::message("actual_native_construct", "original construction cause"))
        });
    let cause = result.err().ok_or("Construction failure lost")?;
    assert_eq!(cause.to_string(), "actual_native_construct: original construction cause");
    assert_eq!(content.calls.iter().filter(|call| **call == "start").count(), 1);
    assert_eq!(content.calls.iter().filter(|call| **call == "close").count(), 1);
    assert_eq!(content.calls.iter().filter(|call| **call == "settle").count(), 2);
    assert!(bootstrap.wake().wake().is_err());
    Ok(())
}
