// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify exact request deduplication, bindings, original deadlines and clock order.
use super::*;

#[test]
fn exact_request_duplicates_never_repeat_effects() -> TestResult {
    let mut f = Fixture::new()?;
    f.environment()?;
    let before = f.session.snapshot();
    assert_eq!(
        f.accept(
            2,
            MainAuthorizationObservation::EnvironmentFolderComplete { identity: f.identity }
        )?,
        None
    );
    assert_eq!(f.session.snapshot(), before);
    f.rejected(
        2,
        MainAuthorizationObservation::RequestedFolderAccepted {
            identity: f.identity,
            next_offset_units: 12,
        },
        rejection(LifecycleRejection::ChangedDuplicate),
    )?;
    Ok(())
}

#[test]
fn stale_requests_cannot_reexecute_omitted_history() -> TestResult {
    let mut f = Fixture::new()?;
    f.accept(
        5,
        MainAuthorizationObservation::RequestedFolderAccepted {
            identity: f.identity,
            next_offset_units: 12,
        },
    )?;
    f.rejected(
        4,
        MainAuthorizationObservation::EnvironmentFolderComplete { identity: f.identity },
        rejection(LifecycleRejection::StaleRequest),
    )?;
    Ok(())
}

#[test]
fn requested_folder_offsets_are_strict_bounded_one_or_two_fragments() -> TestResult {
    let mut f = Fixture::new()?;
    for offset in [0, 16385, 32767] {
        f.rejected(
            1,
            MainAuthorizationObservation::RequestedFolderAccepted {
                identity: f.identity,
                next_offset_units: offset,
            },
            rejection(LifecycleRejection::WrongPhase),
        )?;
    }
    f.accept(
        1,
        MainAuthorizationObservation::RequestedFolderAccepted {
            identity: f.identity,
            next_offset_units: 16384,
        },
    )?;
    f.accept(
        2,
        MainAuthorizationObservation::RequestedFolderAccepted {
            identity: f.identity,
            next_offset_units: 32766,
        },
    )?;
    f.rejected(
        3,
        MainAuthorizationObservation::RequestedFolderAccepted {
            identity: f.identity,
            next_offset_units: 32766,
        },
        rejection(LifecycleRejection::WrongPhase),
    )?;
    f.accept(3, MainAuthorizationObservation::EnvironmentFolderComplete { identity: f.identity })?;
    Ok(())
}

#[test]
fn short_requested_fragment_cannot_be_extended() -> TestResult {
    let mut f = Fixture::new()?;
    f.folder()?;
    f.rejected(
        2,
        MainAuthorizationObservation::RequestedFolderAccepted {
            identity: f.identity,
            next_offset_units: 13,
        },
        rejection(LifecycleRejection::WrongPhase),
    )?;
    Ok(())
}

#[test]
fn wrong_binding_and_stale_main_identity_leave_state_unchanged() -> TestResult {
    let mut f = Fixture::new()?;
    let before = f.session.snapshot();
    let wrong = ClientBinding { connection: ConnectionId::new(2)?, ..f.binding };
    let result = f.session.accept(
        wrong,
        RequestId::new(1)?,
        MainAuthorizationObservation::RequestedFolderAccepted {
            identity: f.identity,
            next_offset_units: 12,
        },
        Moment::from_nanos(10),
    );
    assert!(
        matches!(result, Err(ref e) if e.error == rejection(LifecycleRejection::StaleIdentity))
    );
    assert_eq!(f.session.snapshot(), before);
    let old = MainIdentity { navigation: Generation::new(2)?, ..f.identity };
    f.rejected(
        1,
        MainAuthorizationObservation::RequestedFolderAccepted {
            identity: old,
            next_offset_units: 12,
        },
        rejection(LifecycleRejection::StaleIdentity),
    )?;
    Ok(())
}

#[test]
fn deadline_equality_rejects_duplicates_and_late_proofs_without_rebasing() -> TestResult {
    let mut f = Fixture::new()?;
    f.environment()?;
    let before = f.session.snapshot();
    for now in [100, 101] {
        let result = f.session.accept(
            f.binding,
            RequestId::new(2)?,
            MainAuthorizationObservation::EnvironmentVerified { identity: f.identity },
            Moment::from_nanos(now),
        );
        assert!(matches!(result, Err(ref e) if e.error == MainAuthorizationError::Deadline));
        let duplicate = f.session.accept(
            f.binding,
            RequestId::new(2)?,
            MainAuthorizationObservation::EnvironmentFolderComplete { identity: f.identity },
            Moment::from_nanos(now),
        );
        assert!(matches!(duplicate, Err(ref e) if e.error == MainAuthorizationError::Deadline));
        assert_eq!(f.session.snapshot(), before);
    }
    Ok(())
}

#[test]
fn clock_regression_is_atomic_and_preserves_the_time_error_source() -> TestResult {
    let mut f = Fixture::new()?;
    f.folder()?;
    let result = f.session.accept(
        f.binding,
        RequestId::new(2)?,
        MainAuthorizationObservation::EnvironmentFolderComplete { identity: f.identity },
        Moment::from_nanos(9),
    );
    let rejected = result.err().ok_or_else(|| io::Error::other("regression admitted"))?;
    assert_eq!(rejected.error, MainAuthorizationError::Clock(TimeError::Regression));
    assert!(
        rejected
            .error
            .source()
            .is_some_and(|s| s.downcast_ref::<TimeError>() == Some(&TimeError::Regression))
    );
    assert_eq!(f.session.stage(), MainAuthorizationStage::RequestedFolder);
    assert_eq!(f.session.snapshot().request_highwater, 1);
    Ok(())
}

#[test]
fn proof_continuation_requires_the_same_authenticated_connection() -> TestResult {
    let mut f = Fixture::new()?;
    f.controller()?;
    let before = f.session.snapshot();
    let foreign = ClientBinding { connection: ConnectionId::new(2)?, ..f.binding };
    let result = f.session.accept(
        foreign,
        RequestId::new(3)?,
        MainAuthorizationObservation::BrowserVerified {
            identity: f.identity,
            browser_process_id: pid()?,
        },
        Moment::from_nanos(10),
    );
    assert!(
        matches!(result, Err(ref e) if e.error == rejection(LifecycleRejection::StaleIdentity))
    );
    assert_eq!(f.session.snapshot(), before);
    assert_eq!(
        f.accept(
            3,
            MainAuthorizationObservation::BrowserVerified {
                identity: f.identity,
                browser_process_id: pid()?,
            }
        )?,
        Some(MainAuthorizationEffect::AuthorizeBrowser {
            identity: f.identity,
            browser_process_id: pid()?,
        }),
    );
    Ok(())
}

#[test]
fn exhausted_request_sequence_never_wraps_or_invents_retirement() -> TestResult {
    let mut f = Fixture::new()?;
    f.accept(
        u64::MAX,
        MainAuthorizationObservation::RequestedFolderAccepted {
            identity: f.identity,
            next_offset_units: 12,
        },
    )?;
    f.rejected(
        1,
        MainAuthorizationObservation::EnvironmentFolderComplete { identity: f.identity },
        rejection(LifecycleRejection::StaleRequest),
    )?;
    f.session.retire(f.identity)?;
    f.rejected(
        u64::MAX,
        MainAuthorizationObservation::MainRetired { identity: f.identity },
        rejection(LifecycleRejection::ChangedDuplicate),
    )?;
    assert_eq!(f.session.confirm_retired(f.identity), Err(LifecycleRejection::WrongPhase));
    assert_eq!(f.session.snapshot().request_highwater, u64::MAX);
    Ok(())
}
