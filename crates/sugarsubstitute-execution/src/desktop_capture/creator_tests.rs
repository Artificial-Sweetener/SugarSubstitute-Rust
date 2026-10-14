// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bound the campaign solely by immutable authenticated enrollment facts.
use super::*;
use crate::{
    admission::Core,
    runtime_test_support::{ClockPort, ManualClock, TestResult, Wake},
};
use std::sync::Arc;
use sugarsubstitute_contracts::{execution::*, lifecycle::*};
fn creator(
    core: &Core,
    enrollment: crate::lifecycle_pipe::LifecyclePipeDeadline,
) -> TestResult<AuthenticatedDesktopCreator> {
    Ok(AuthenticatedDesktopCreator {
        run: RunKey {
            owner: CoordinatorKey {
                namespace: NamespaceId::new([1; 32]),
                epoch: CoordinatorEpoch::new([2; 16])?,
            },
            instance: InstanceId::new(1)?,
            generation: RunGeneration::new(1)?,
        },
        child: (3, 3),
        creator: (1, 1),
        handler: (2, 2),
        core_identity: core as *const Core as usize,
        native_scope: ScopeId::new(3)?,
        enrollment,
    })
}
fn core() -> TestResult<Arc<Core>> {
    Ok(Core::new(
        ExecutionId::new(1)?,
        Arc::new(ClockPort(Arc::new(ManualClock::default()))),
        Arc::new(Wake::default()),
    ))
}
#[test]
fn preview_original_enrollment_bounds_campaign_without_observing_a_new_start() -> TestResult {
    let core = core()?;
    let creator = creator(&core, crate::lifecycle_pipe::LifecyclePipeDeadline::new(300, 10)?)?;
    assert!(creator.permits_campaign(CounterDeadline::new(2100, 10)?));
    assert!(!creator.permits_campaign(CounterDeadline::new(2101, 10)?));
    assert!(!creator.permits_campaign(CounterDeadline::new(2100, 11)?));
    let narrowed = AuthenticatedDesktopCreator {
        enrollment: crate::lifecycle_pipe::LifecyclePipeDeadline::new(200, 10)?,
        ..creator
    };
    assert!(narrowed.permits_campaign(CounterDeadline::new(2000, 10)?));
    assert!(!narrowed.permits_campaign(CounterDeadline::new(2001, 10)?));
    Ok(())
}
#[test]
fn preview_enrollment_ceiling_overflow_is_rejected_before_conversion() -> TestResult {
    let core = core()?;
    for enrollment in [
        crate::lifecycle_pipe::LifecyclePipeDeadline::new(i64::MAX, 1)?,
        crate::lifecycle_pipe::LifecyclePipeDeadline::new(1, i64::MAX)?,
    ] {
        assert!(
            !creator(&core, enrollment)?
                .permits_campaign(CounterDeadline::new(1, enrollment.frequency())?)
        );
    }
    Ok(())
}
#[test]
fn preview_matching_numeric_execution_and_scope_do_not_match_foreign_native_owner() -> TestResult {
    let original = core()?;
    let foreign = core()?;
    let creator = creator(&original, crate::lifecycle_pipe::LifecyclePipeDeadline::new(300, 10)?)?;
    assert!(creator.matches_native(&original, ScopeId::new(3)?));
    assert!(!creator.matches_native(&foreign, ScopeId::new(3)?));
    assert!(!creator.matches_native(&original, ScopeId::new(4)?));
    Ok(())
}
