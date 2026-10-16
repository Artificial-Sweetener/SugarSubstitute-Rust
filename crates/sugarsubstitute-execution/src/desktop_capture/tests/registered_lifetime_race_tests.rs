// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Select shutdown interleavings on the actual admitted registered owner.
use super::*;
use crate::capture_attach::control::CaptureCleanupSnapshot;

/// A completed genuine request can overtake the worker's earlier stopped=false read.
#[test]
fn registered_lifetime_stop_preserves_cleanup_that_overtook_live_observation() -> TestResult {
    let mut owner = RegisteredOwner::new(None)?;
    owner.register()?;
    assert!(!owner.active.work.shared.cleanup.stopped());
    let original = owner.session.control().request_cleanup();
    let observed = crate::desktop_capture::registered_lifetime::stop(
        &owner.core,
        &owner.active.work,
        &owner.active.context,
        Moment::from_nanos(0),
    )?;
    assert!(matches!(observed, Some(crate::task::Stop::Cancelled)));
    let (
        CaptureCleanupSnapshot::Latched { budget: before, counter: Ok(first) },
        CaptureCleanupSnapshot::Latched { budget: after, counter: Ok(second) },
    ) = (original, owner.session.control().cleanup_snapshot())
    else {
        return Err("Actual first cleanup observation missing".into());
    };
    assert_eq!(before, after);
    assert_eq!(first, second);
    owner.settle()
}

/// Select the exact state published by request's first fence before its clock claim.
#[test]
fn registered_lifetime_stop_preserves_original_reason_before_cleanup_measurement() -> TestResult {
    let mut owner = RegisteredOwner::new(None)?;
    owner.register()?;
    {
        let mut state = lock(&owner.core.state);
        let cleanup = lock(&owner.active.work.shared.cleanup.state);
        let mut stage = lock(&owner.active.work.shared.state);
        assert!(!cleanup.stopped);
        assert!(matches!(cleanup.snapshot, CaptureCleanupSnapshot::Dormant));
        stage.phase = control::DesktopCapturePhase::Cleaning;
        stage.stopped_outcome = Some(crate::task::Stop::TimedOut);
        state
            .jobs
            .get_mut(&owner.active.context.job_id())
            .and_then(|record| record.native_deadlines.as_mut())
            .ok_or("Actual registered deadlines missing")?
            .stop();
    }
    let observed = crate::desktop_capture::registered_lifetime::stop(
        &owner.core,
        &owner.active.work,
        &owner.active.context,
        Moment::from_nanos(0),
    )?;
    assert!(matches!(observed, Some(crate::task::Stop::TimedOut)));
    assert!(!owner.active.work.shared.cleanup.stopped());
    assert!(matches!(owner.session.control().cleanup_snapshot(), CaptureCleanupSnapshot::Dormant));
    owner.session.control().request_cleanup();
    assert!(matches!(
        lock(&owner.active.work.shared.state).stopped_outcome,
        Some(crate::task::Stop::TimedOut)
    ));
    owner.settle()
}

/// Cleanup recognition cannot repair a missing reason, registration, phase or record.
#[test]
fn registered_lifetime_stop_rejects_unqualified_cleanup_states() -> TestResult {
    for invalid in 0..5 {
        let mut owner = RegisteredOwner::new(None)?;
        owner.register()?;
        {
            let mut state = lock(&owner.core.state);
            let _cleanup = lock(&owner.active.work.shared.cleanup.state);
            let mut stage = lock(&owner.active.work.shared.state);
            stage.phase = control::DesktopCapturePhase::Cleaning;
            stage.stopped_outcome = Some(crate::task::Stop::Cancelled);
            match invalid {
                0 => stage.stopped_outcome = None,
                1 => stage.registered_published = false,
                2 => stage.phase = control::DesktopCapturePhase::Preparing,
                3 => stage.phase = control::DesktopCapturePhase::Registered,
                _ => {
                    state
                        .jobs
                        .get_mut(&owner.active.context.job_id())
                        .ok_or("Actual registration record missing")?
                        .retiring = true;
                }
            }
        }
        let observed = crate::desktop_capture::registered_lifetime::stop(
            &owner.core,
            &owner.active.work,
            &owner.active.context,
            Moment::from_nanos(0),
        );
        assert!(matches!(observed, Err(AdmissionError::InvalidCaptureAdmission)));
        {
            let mut state = lock(&owner.core.state);
            let _cleanup = lock(&owner.active.work.shared.cleanup.state);
            let mut stage = lock(&owner.active.work.shared.state);
            stage.phase = control::DesktopCapturePhase::Registered;
            stage.stopped_outcome = None;
            stage.registered_published = true;
            state
                .jobs
                .get_mut(&owner.active.context.job_id())
                .ok_or("Actual registration record missing")?
                .retiring = false;
        }
        owner.session.control().request_cleanup();
        owner.settle()?;
    }
    Ok(())
}
