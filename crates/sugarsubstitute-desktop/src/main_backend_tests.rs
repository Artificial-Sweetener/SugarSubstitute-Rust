// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the actual fixed-slot/revision owner without native windows or backend fakes.
use super::*;
use sugarsubstitute_contracts::{execution::Generation, lifecycle::*, startup::LaunchId};
type TestResult = Result<(), Box<dyn std::error::Error>>;
fn identity() -> Result<MainIdentity, Box<dyn std::error::Error>> {
    Ok(MainIdentity {
        run: RunKey {
            owner: CoordinatorKey {
                namespace: NamespaceId::new([1; 32]),
                epoch: CoordinatorEpoch::new([2; 16])?,
            },
            instance: InstanceId::new(1)?,
            generation: RunGeneration::new(1)?,
        },
        launch: LaunchId::new(1)?,
        navigation: Generation::new(1)?,
    })
}
fn lease(identity: MainIdentity, number: u64) -> Result<LeaseKey, Box<dyn std::error::Error>> {
    Ok(LeaseKey {
        run: identity.run,
        resource: ResourceKey {
            owner: identity.run.owner,
            id: ResourceId::new(1)?,
            generation: ResourceGeneration::new(number)?,
        },
        connection: ConnectionId::new(1)?,
        id: LeaseId::new(number)?,
    })
}
fn labels() -> MainRecoveryLabels {
    MainRecoveryLabels { retry: "Retry", retry_ready: "Ready to retry" }
}
#[test]
fn initial_outage_control_is_hidden_disabled_without_a_failed_reference() -> TestResult {
    let owner = Recovery::new(identity()?);
    let initial = owner.presentation(labels());
    assert!(!initial.visible);
    assert!(initial.action.is_none());
    assert_eq!(initial.label, labels().retry);
    assert!(owner.outage.is_none());
    Ok(())
}
#[test]
fn presentation_requires_complete_current_main_and_matching_failed_run_owner() -> TestResult {
    let current = identity()?;
    let failed = lease(current, 1)?;
    let mut owner = Recovery::new(current);
    let mut foreign = current;
    foreign.navigation = Generation::new(2)?;
    assert_eq!(
        owner.present(foreign, failed, MainBackendState::RetryReady),
        Err(RecoveryError::Identity)
    );
    foreign = current;
    foreign.launch = LaunchId::new(2)?;
    assert_eq!(
        owner.present(foreign, failed, MainBackendState::RetryReady),
        Err(RecoveryError::Identity)
    );
    foreign = current;
    foreign.run.owner.epoch = CoordinatorEpoch::new([3; 16])?;
    assert_eq!(
        owner.present(foreign, failed, MainBackendState::RetryReady),
        Err(RecoveryError::Identity)
    );
    let mut wrong = failed;
    wrong.run = foreign.run;
    assert_eq!(
        owner.present(current, wrong, MainBackendState::RetryReady),
        Err(RecoveryError::Identity)
    );
    wrong = failed;
    wrong.resource.owner = foreign.run.owner;
    assert_eq!(
        owner.present(current, wrong, MainBackendState::RetryReady),
        Err(RecoveryError::Identity)
    );
    assert!(owner.outage.is_none());
    assert_eq!(owner.revision, 0);
    Ok(())
}
#[test]
fn native_click_consumes_ready_once_and_reports_only_original_main_failed_pair() -> TestResult {
    let current = identity()?;
    let failed = lease(current, 1)?;
    let mut owner = Recovery::new(current);
    owner.present(current, failed, MainBackendState::RetryReady)?;
    let action = owner.action().ok_or("Retry action missing")?;
    let observation = owner.consume(action)?.ok_or("Native click not consumed")?;
    let DesktopStartupObservation::BackendRetryRequested { identity, failed: actual } = observation
    else {
        return Err("Unexpected native observation".into());
    };
    assert_eq!((identity, actual), (current, failed));
    assert!(owner.action().is_none());
    assert!(owner.presentation(labels()).visible);
    assert_eq!(owner.presentation(labels()).label, labels().retry);
    assert!(owner.consume(action)?.is_none());
    Ok(())
}
#[test]
fn queued_click_from_same_failed_lease_before_rearmed_ready_is_ignored() -> TestResult {
    let current = identity()?;
    let failed = lease(current, 1)?;
    let mut owner = Recovery::new(current);
    owner.present(current, failed, MainBackendState::RetryReady)?;
    let old = owner.action().ok_or("Old action missing")?;
    let _first_request = owner.consume(old)?;
    owner.present(current, failed, MainBackendState::RetryReady)?;
    let fresh = owner.action().ok_or("Fresh action missing")?;
    assert_ne!(old, fresh);
    assert!(owner.consume(old)?.is_none());
    assert!(owner.consume(fresh)?.is_some());
    Ok(())
}
#[test]
fn recovered_requires_exact_old_lease_and_retains_replay_tombstone() -> TestResult {
    let current = identity()?;
    let first = lease(current, 1)?;
    let next = lease(current, 2)?;
    let mut owner = Recovery::new(current);
    assert_eq!(
        owner.present(current, first, MainBackendState::Recovered),
        Err(RecoveryError::StaleLease)
    );
    owner.present(current, first, MainBackendState::RetryReady)?;
    let old = owner.action().ok_or("Old action missing")?;
    assert_eq!(
        owner.present(current, next, MainBackendState::Recovered),
        Err(RecoveryError::StaleLease)
    );
    assert_eq!(
        owner.present(current, next, MainBackendState::RetryReady),
        Err(RecoveryError::StaleLease)
    );
    owner.present(current, first, MainBackendState::Recovered)?;
    assert!(!owner.presentation(labels()).visible);
    assert_eq!(
        owner.present(current, first, MainBackendState::RetryReady),
        Err(RecoveryError::StaleLease)
    );
    assert!(!owner.present(current, first, MainBackendState::Recovered)?);
    owner.present(current, next, MainBackendState::RetryReady)?;
    assert!(owner.consume(old)?.is_none());
    assert_eq!(owner.action().ok_or("New outage missing")?.failed, next);
    Ok(())
}
#[test]
fn closing_and_exhausted_revision_never_reset_or_wrap_action_identity() -> TestResult {
    let current = identity()?;
    let failed = lease(current, 1)?;
    let mut owner = Recovery::new(current);
    owner.present(current, failed, MainBackendState::RetryReady)?;
    owner.revision = u64::MAX;
    let action = owner.action().ok_or("Max revision action missing")?;
    assert!(matches!(owner.consume(action), Err(RecoveryError::RevisionExhausted)));
    assert_eq!(owner.revision, u64::MAX);
    assert!(owner.action().is_none());
    assert!(!owner.presentation(labels()).visible);
    assert_eq!(
        owner.present(current, failed, MainBackendState::RetryReady),
        Err(RecoveryError::Closed)
    );
    let mut closed = Recovery::new(current);
    closed.close();
    assert_eq!(
        closed.present(current, failed, MainBackendState::RetryReady),
        Err(RecoveryError::Closed)
    );
    Ok(())
}
#[test]
fn caption_fourth_peer_keeps_original_close_minimize_maximize_and_resize_geometry() -> TestResult {
    let captions = crate::MainCaptionLabels {
        minimize: "min",
        maximize: "max",
        restore: "restore",
        close: "close",
    };
    for (width, maximized) in [(304.0, false), (1280.0, false), (1600.0, true)] {
        let controls = caption_controls(
            width,
            maximized,
            captions,
            Recovery::new(identity()?).presentation(labels()),
        );
        assert_eq!(controls.map(|control| control.id), [1, 2, 3, 4]);
        for (index, control) in controls[..3].iter().enumerate() {
            assert_eq!(control.bounds.x, width - 144.0 + index as f64 * 48.0);
            assert_eq!(control.bounds.width, 48.0);
            assert_eq!(control.close, index == 2);
        }
        assert_eq!(controls[1].label, if maximized { captions.restore } else { captions.maximize });
        assert_eq!(controls[3].bounds.x, 8.0);
        assert!(controls[3].bounds.x + controls[3].bounds.width <= controls[0].bounds.x);
        assert!(!controls[3].close);
    }
    Ok(())
}
#[test]
fn duplicate_presentation_is_inert_and_shared_retrying_has_no_click_mapping() -> TestResult {
    let current = identity()?;
    let failed = lease(current, 1)?;
    let mut owner = Recovery::new(current);
    owner.present(current, failed, MainBackendState::Retiring)?;
    assert!(!owner.presentation(labels()).visible);
    let revision = owner.revision;
    assert!(!owner.present(current, failed, MainBackendState::Retiring)?);
    assert_eq!(owner.revision, revision);
    owner.present(current, failed, MainBackendState::Retrying)?;
    assert!(owner.presentation(labels()).visible);
    assert!(owner.action().is_none());
    Ok(())
}

#[test]
fn explicit_lineage_replaces_pending_outage_without_false_recovery_or_stale_clicks() -> TestResult {
    let current = identity()?;
    let first = lease(current, 1)?;
    let next = lease(current, 2)?;
    let mut owner = Recovery::new(current);
    owner.present(current, first, MainBackendState::RetryReady)?;
    let old_action = owner.action().ok_or("Original action missing")?;
    let _request = owner.consume(old_action)?;
    assert_eq!(
        owner.replace(current, first, next, MainBackendState::Recovered),
        Err(RecoveryError::StaleLease)
    );
    assert_eq!(
        owner.replace(current, next, first, MainBackendState::Retiring),
        Err(RecoveryError::StaleLease)
    );
    let mut foreign = next;
    foreign.connection = ConnectionId::new(2)?;
    assert_eq!(
        owner.replace(current, first, foreign, MainBackendState::Retiring),
        Err(RecoveryError::StaleLease)
    );
    owner.replace(current, first, next, MainBackendState::Retiring)?;
    assert_eq!(owner.outage, Some((next, MainBackendState::Retiring)));
    assert!(owner.consume(old_action)?.is_none());
    assert_eq!(
        owner.present(current, first, MainBackendState::Recovered),
        Err(RecoveryError::StaleLease)
    );
    owner.present(current, next, MainBackendState::RetryReady)?;
    assert_eq!(owner.action().ok_or("New failed-interest action missing")?.failed, next);
    Ok(())
}
