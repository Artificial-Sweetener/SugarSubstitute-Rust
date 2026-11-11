// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify monotonic retirement, successor admission and retained failure causes.
use super::*;

#[test]
fn retirement_fence_rejects_queued_and_late_proof_continuations() -> TestResult {
    let mut f = Fixture::new()?;
    f.environment()?;
    f.session.retire(f.identity)?;
    f.session.retire(f.identity)?;
    f.rejected(
        2,
        MainAuthorizationObservation::EnvironmentVerified { identity: f.identity },
        rejection(LifecycleRejection::WrongPhase),
    )?;
    assert_eq!(f.session.confirm_retired(f.identity), Err(LifecycleRejection::WrongPhase));
    assert_eq!(
        f.session.begin(f.identity, f.lease, Moment::from_nanos(200)),
        Err(LifecycleRejection::WrongPhase)
    );
    Ok(())
}

#[test]
fn actual_retirement_observation_and_separate_root_confirmation_are_required() -> TestResult {
    let mut f = Fixture::new()?;
    f.rejected(
        1,
        MainAuthorizationObservation::MainRetired { identity: f.identity },
        rejection(LifecycleRejection::WrongPhase),
    )?;
    f.session.retire(f.identity)?;
    assert_eq!(
        f.session.accept(
            f.binding,
            RequestId::new(1)?,
            MainAuthorizationObservation::MainRetired { identity: f.identity },
            Moment::from_nanos(150)
        )?,
        Some(MainAuthorizationEffect::AcknowledgeRetirement(f.identity))
    );
    assert_eq!(f.session.stage(), MainAuthorizationStage::Retiring);
    assert_eq!(f.session.snapshot().frontend_until, Some(Moment::from_nanos(100)));
    f.session.confirm_retired(f.identity)?;
    assert_eq!(f.session.stage(), MainAuthorizationStage::Retired);
    assert_eq!(
        f.session.accept(
            f.binding,
            RequestId::new(1)?,
            MainAuthorizationObservation::MainRetired { identity: f.identity },
            Moment::from_nanos(151)
        )?,
        None
    );
    Ok(())
}

#[test]
fn rejected_close_keeps_retiring_and_blocks_same_or_new_attempt() -> TestResult {
    let mut f = Fixture::new()?;
    f.controller()?;
    f.session.retire(f.identity)?;
    // No successful MainRetired was observed after a native close rejection.
    assert_eq!(f.session.confirm_retired(f.identity), Err(LifecycleRejection::WrongPhase));
    let newer =
        MainIdentity { launch: LaunchId::new(2)?, navigation: Generation::new(2)?, ..f.identity };
    assert_eq!(
        f.session.begin(newer, f.lease, Moment::from_nanos(200)),
        Err(LifecycleRejection::WrongPhase)
    );
    f.rejected(
        3,
        MainAuthorizationObservation::BrowserVerified {
            identity: f.identity,
            browser_process_id: pid()?,
        },
        rejection(LifecycleRejection::WrongPhase),
    )?;
    Ok(())
}

#[test]
fn failed_close_retains_first_cause_and_does_not_reopen_on_transfer() -> TestResult {
    let mut f = Fixture::new()?;
    f.controller()?;
    f.session.retire(f.identity)?;
    f.accept(4, failure(f.identity, LifecycleFailureCode::Cleanup, "original close cause"))?;
    assert_eq!(f.session.stage(), MainAuthorizationStage::Retiring);
    assert_eq!(
        f.session.failure().and_then(Error::source).map(ToString::to_string),
        Some("original close cause".into())
    );
    let second = f.session.accept(
        f.binding,
        RequestId::new(5)?,
        failure(f.identity, LifecycleFailureCode::Cleanup, "second close cause"),
        Moment::from_nanos(10),
    );
    let rejected = second.err().ok_or_else(|| io::Error::other("second cause replaced first"))?;
    if let MainAuthorizationObservation::Failed { failure, .. } = rejected.observation {
        assert_eq!(failure.source().map(ToString::to_string), Some("second close cause".into()));
    } else {
        return Err(io::Error::other("rejected cause ownership lost").into());
    }
    let cause = f.session.take_failure().ok_or_else(|| io::Error::other("first cause lost"))?;
    assert_eq!(cause.source().map(ToString::to_string), Some("original close cause".into()));
    assert!(f.session.snapshot().failure_observed);
    f.rejected(
        3,
        MainAuthorizationObservation::BrowserVerified {
            identity: f.identity,
            browser_process_id: pid()?,
        },
        rejection(LifecycleRejection::WrongPhase),
    )?;
    assert_eq!(f.session.confirm_retired(f.identity), Err(LifecycleRejection::WrongPhase));
    Ok(())
}

#[test]
fn first_failure_closes_admission_before_retirement_and_survives_until_observed() -> TestResult {
    let mut f = Fixture::new()?;
    f.environment()?;
    f.accept(
        2,
        failure(f.identity, LifecycleFailureCode::Preparation, "environment native cause"),
    )?;
    assert_eq!(f.session.stage(), MainAuthorizationStage::Failed);
    f.rejected(
        2,
        MainAuthorizationObservation::EnvironmentVerified { identity: f.identity },
        rejection(LifecycleRejection::WrongPhase),
    )?;
    f.session.retire(f.identity)?;
    f.accept(3, MainAuthorizationObservation::MainRetired { identity: f.identity })?;
    f.session.confirm_retired(f.identity)?;
    let newer =
        MainIdentity { launch: LaunchId::new(2)?, navigation: Generation::new(2)?, ..f.identity };
    assert_eq!(
        f.session.begin(newer, f.lease, Moment::from_nanos(200)),
        Err(LifecycleRejection::WrongPhase)
    );
    let _original =
        f.session.take_failure().ok_or_else(|| io::Error::other("failure not retained"))?;
    f.session.begin(newer, f.lease, Moment::from_nanos(200))?;
    assert!(!f.session.snapshot().failure_observed);
    Ok(())
}

#[test]
fn successor_requires_both_increasing_attempt_ids_and_preserves_request_highwater() -> TestResult {
    let mut f = Fixture::new()?;
    f.folder()?;
    f.session.retire(f.identity)?;
    f.accept(10, MainAuthorizationObservation::MainRetired { identity: f.identity })?;
    f.session.confirm_retired(f.identity)?;
    for (launch, navigation) in [(1, 2), (2, 1)] {
        let candidate = MainIdentity {
            launch: LaunchId::new(launch)?,
            navigation: Generation::new(navigation)?,
            ..f.identity
        };
        assert_eq!(
            f.session.begin(candidate, f.lease, Moment::from_nanos(200)),
            Err(LifecycleRejection::StaleIdentity)
        );
    }
    let newer =
        MainIdentity { launch: LaunchId::new(2)?, navigation: Generation::new(2)?, ..f.identity };
    f.session.begin(newer, f.lease, Moment::from_nanos(200))?;
    f.identity = newer;
    f.rejected(
        10,
        MainAuthorizationObservation::RequestedFolderAccepted {
            identity: newer,
            next_offset_units: 12,
        },
        rejection(LifecycleRejection::StaleRequest),
    )?;
    f.accept(
        11,
        MainAuthorizationObservation::RequestedFolderAccepted {
            identity: newer,
            next_offset_units: 12,
        },
    )?;
    assert_eq!(f.session.snapshot().request_highwater, 11);
    Ok(())
}

#[test]
fn full_lease_and_session_binding_are_checked_before_begin() -> TestResult {
    let f = Fixture::new()?;
    let mut empty = MainAuthorizationSession::new(f.identity.run);
    let foreign = CoordinatorKey { namespace: NamespaceId::new([8; 32]), ..f.identity.run.owner };
    let lease =
        LeaseKey { resource: ResourceKey { owner: foreign, ..f.lease.resource }, ..f.lease };
    assert_eq!(
        empty.begin(f.identity, lease, Moment::from_nanos(100)),
        Err(LifecycleRejection::ForeignOwner)
    );
    let wrong_run = RunKey { instance: InstanceId::new(2)?, ..f.identity.run };
    assert_eq!(
        empty.begin(f.identity, LeaseKey { run: wrong_run, ..f.lease }, Moment::from_nanos(100)),
        Err(LifecycleRejection::StaleIdentity)
    );
    assert_eq!(empty.snapshot().identity, None);
    Ok(())
}

#[test]
fn late_or_foreign_failure_returns_the_exact_owned_cause_without_mutation() -> TestResult {
    let mut f = Fixture::new()?;
    let other = MainIdentity { navigation: Generation::new(2)?, ..f.identity };
    let before = f.session.snapshot();
    let result = f.session.accept(
        f.binding,
        RequestId::new(1)?,
        failure(other, LifecycleFailureCode::Preparation, "foreign cause"),
        Moment::from_nanos(1000),
    );
    let rejected = result.err().ok_or_else(|| io::Error::other("foreign failure admitted"))?;
    assert_eq!(rejected.error, rejection(LifecycleRejection::StaleIdentity));
    if let MainAuthorizationObservation::Failed { failure, .. } = rejected.observation {
        assert_eq!(failure.source().map(ToString::to_string), Some("foreign cause".into()));
    } else {
        return Err(io::Error::other("foreign original cause lost").into());
    }
    assert_eq!(f.session.snapshot(), before);
    Ok(())
}
