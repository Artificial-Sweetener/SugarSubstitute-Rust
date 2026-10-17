// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the actual creation fence with typed external custody facts, without windows.
use super::*;
use sugarsubstitute_contracts::{execution::Generation, lifecycle::*, startup::LaunchId};
type TestResult = Result<(), Box<dyn std::error::Error>>;
fn identity(launch: u64) -> Result<MainIdentity, Box<dyn std::error::Error>> {
    Ok(MainIdentity {
        run: RunKey {
            owner: CoordinatorKey {
                namespace: NamespaceId::new([1; 32]),
                epoch: CoordinatorEpoch::new([2; 16])?,
            },
            instance: InstanceId::new(1)?,
            generation: RunGeneration::new(1)?,
        },
        launch: LaunchId::new(launch)?,
        navigation: Generation::new(1)?,
    })
}
#[test]
fn exact_absence_fences_queued_and_future_same_attempt_creation_before_observation() -> TestResult {
    let id = identity(1)?;
    let mut fence = MainFence::new();
    let queued_before_check = id;
    assert!(fence.prepare(id, Some(id)).is_ok());
    fence.absent(id, Some(id), false, MainCustody::Absent)?;
    assert!(fence.prepare(queued_before_check, Some(id)).is_err());
    assert!(fence.prepare(id, Some(id)).is_err());
    assert!(fence.absent(id, Some(id), false, MainCustody::Absent).is_err());
    Ok(())
}
#[test]
fn live_partial_callback_and_application_constructor_custody_prevent_absence() -> TestResult {
    let id = identity(1)?;
    let mut fence = MainFence::new();
    // The adapter samples any retained actor as Outstanding until its actual checked
    // retirement, covering live, partial constructor and in-flight callback custody alike.
    for _stage in ["live", "partial constructor", "callback pending"] {
        assert!(fence.absent(id, Some(id), false, MainCustody::Outstanding).is_err());
        assert!(fence.prepare(id, Some(id)).is_ok());
    }
    assert!(fence.absent(id, Some(id), true, MainCustody::Absent).is_err());
    assert!(fence.absent(id, Some(id), false, MainCustody::CheckedRetired(id)).is_err());
    assert!(fence.prepare(id, Some(id)).is_ok());
    Ok(())
}
#[test]
fn stale_foreign_and_unregistered_successor_identity_cannot_close_current_attempt() -> TestResult {
    let current = identity(2)?;
    let mut fence = MainFence::new();
    let mut foreign = current;
    foreign.run.instance = InstanceId::new(2)?;
    for requested in [identity(1)?, identity(3)?, foreign] {
        assert!(fence.absent(requested, Some(current), false, MainCustody::Absent).is_err());
    }
    assert!(fence.absent(current, None, false, MainCustody::Absent).is_err());
    fence.absent(current, Some(current), false, MainCustody::CheckedRetired(identity(1)?))?;
    assert!(fence.prepare(current, Some(current)).is_err());
    assert!(fence.prepare(identity(3)?, Some(identity(3)?)).is_ok());
    Ok(())
}
#[test]
fn callback_quiescence_and_checked_predecessor_order_are_required_before_successor_absence()
-> TestResult {
    let prior = identity(1)?;
    let current = identity(2)?;
    let mut fence = MainFence::new();
    assert!(fence.absent(current, Some(current), false, MainCustody::Outstanding).is_err());
    assert!(
        fence
            .absent(current, Some(current), false, MainCustody::CheckedRetired(identity(3)?))
            .is_err()
    );
    fence.absent(current, Some(current), false, MainCustody::CheckedRetired(prior))?;
    assert!(fence.prepare(current, Some(current)).is_err());
    Ok(())
}

/// Inert outer failure/settlement/native-custody facts exercise the real application
/// retry and current-identity APIs plus the creating-thread fence, without native proof.
#[test]
fn actual_configured_retry_can_reset_navigation_before_absence_of_the_new_launch() -> TestResult {
    use sugarsubstitute_application::startup::{StartupPhase, StartupRejection, StartupSession};
    use sugarsubstitute_contracts::{
        configuration::{
            ConnectionConfiguration, LoopbackOrigin, Presentation, ProfileId, StartupConfiguration,
            Theme,
        },
        startup::{StartupEvent, StartupFailure, StartupFailureCode, StartupObservation},
    };
    let mut prior = identity(7)?;
    prior.navigation = Generation::new(91)?;
    let configuration = StartupConfiguration::new(
        ConnectionConfiguration::Attach(LoopbackOrigin::parse("http://127.0.0.1:8188", 8188)?),
        ProfileId::new("RetryFence".into())?,
        Presentation::new("en", Theme::Dark, true)?,
    );
    let mut session = StartupSession::new(prior.launch, prior.navigation, configuration);
    session.observe(StartupEvent::new(prior.launch, StartupObservation::SplashPresented))?;
    session.observe(StartupEvent::new(
        prior.launch,
        StartupObservation::Failed(StartupFailure::new(
            StartupFailureCode::ConnectionUnavailable,
            Box::new(std::io::Error::other("outer adapter failure")),
        )),
    ))?;
    let retry = LaunchId::new(8)?;
    assert_eq!(
        session.retry_configured(retry, Generation::new(1)?),
        Err(StartupRejection::SettlementRequired)
    );
    session.observe(StartupEvent::new(prior.launch, StartupObservation::ConnectionSettled))?;
    assert_eq!(session.phase(), StartupPhase::Stopped);
    session.retry_configured(retry, Generation::new(1)?)?;
    let current =
        MainIdentity { run: prior.run, launch: session.launch(), navigation: session.navigation() };
    assert_eq!(current.launch, retry);
    assert_eq!(current.navigation, Generation::new(1)?);
    let mut fence = MainFence::new();
    let queued_before_absence = current;
    fence.prepare(queued_before_absence, Some(current))?;
    let mut foreign = prior;
    foreign.run.instance = InstanceId::new(2)?;
    let same_launch = MainIdentity { navigation: Generation::new(999)?, ..current };
    for retired in [same_launch, identity(9)?, foreign] {
        assert!(
            fence
                .absent(current, Some(current), false, MainCustody::CheckedRetired(retired))
                .is_err()
        );
        fence.prepare(current, Some(current))?;
    }
    assert!(
        fence.absent(current, Some(current), true, MainCustody::CheckedRetired(prior)).is_err()
    );
    assert!(fence.absent(current, Some(current), false, MainCustody::Outstanding).is_err());
    assert!(
        fence
            .absent(same_launch, Some(current), false, MainCustody::CheckedRetired(prior))
            .is_err()
    );
    assert!(fence.absent(identity(9)?, Some(current), false, MainCustody::Absent).is_err());
    fence.absent(current, Some(current), false, MainCustody::CheckedRetired(prior))?;
    assert!(fence.prepare(queued_before_absence, Some(current)).is_err());
    assert!(fence.prepare(current, Some(current)).is_err());
    assert!(fence.absent(current, Some(current), false, MainCustody::Absent).is_err());
    Ok(())
}

#[test]
fn frontend_replacement_requires_checked_old_custody_and_actual_splash_retirement() -> TestResult {
    let old = identity(1)?;
    let mut next = identity(2)?;
    next.navigation = Generation::new(2)?;
    let fence = MainFence::new();
    assert!(fence.replacement(next, Some(next), MainCustody::CheckedRetired(old), true)?);
    assert!(!fence.replacement(next, Some(next), MainCustody::Outstanding, true)?);
    assert!(!fence.replacement(next, Some(next), MainCustody::CheckedRetired(old), false)?);
    assert!(!fence.replacement(next, Some(next), MainCustody::Absent, true)?);
    assert!(fence.replacement(next, Some(old), MainCustody::CheckedRetired(old), true).is_err());
    next.navigation = old.navigation;
    assert!(!fence.replacement(next, Some(next), MainCustody::CheckedRetired(old), true)?);
    Ok(())
}
