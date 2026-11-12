// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Browser authorization alone cannot establish a revealed, ready application.
use super::*;

fn authorized(f: &mut Fixture) -> TestResult {
    f.controller()?;
    f.accept(
        3,
        MainAuthorizationObservation::BrowserVerified {
            identity: f.identity,
            browser_process_id: pid()?,
        },
    )?;
    f.accept(
        3,
        MainAuthorizationObservation::BrowserAuthorizationIssued {
            identity: f.identity,
            browser_process_id: pid()?,
        },
    )?;
    Ok(())
}

#[test]
fn reveal_requires_actual_prior_authorization_and_is_exactly_once() -> TestResult {
    let mut f = Fixture::new()?;
    f.rejected(
        1,
        MainAuthorizationObservation::MainRevealed { identity: f.identity },
        rejection(LifecycleRejection::WrongPhase),
    )?;
    authorized(&mut f)?;
    assert!(!f.session.snapshot().reveal_observed);
    f.accept(4, MainAuthorizationObservation::MainRevealed { identity: f.identity })?;
    assert!(f.session.snapshot().reveal_observed);
    let original = f.session.snapshot();
    f.accept(4, MainAuthorizationObservation::MainRevealed { identity: f.identity })?;
    assert_eq!(f.session.snapshot(), original);
    f.rejected(
        5,
        MainAuthorizationObservation::MainRevealed { identity: f.identity },
        rejection(LifecycleRejection::WrongPhase),
    )?;
    Ok(())
}

#[test]
fn reveal_has_no_new_global_startup_deadline_and_keeps_identity_and_clock_checks() -> TestResult {
    let mut f = Fixture::new()?;
    authorized(&mut f)?;
    let mut foreign = f.identity;
    foreign.navigation = Generation::new(2)?;
    assert!(
        f.session
            .accept(
                f.binding,
                RequestId::new(4)?,
                MainAuthorizationObservation::MainRevealed { identity: foreign },
                Moment::from_nanos(1000)
            )
            .is_err()
    );
    assert!(!f.session.snapshot().reveal_observed);
    f.session.accept(
        f.binding,
        RequestId::new(4)?,
        MainAuthorizationObservation::MainRevealed { identity: f.identity },
        Moment::from_nanos(1000),
    )?;
    assert!(f.session.snapshot().reveal_observed);
    assert!(
        f.session
            .accept(
                f.binding,
                RequestId::new(4)?,
                MainAuthorizationObservation::MainRevealed { identity: f.identity },
                Moment::from_nanos(999)
            )
            .is_err()
    );
    assert_eq!(f.session.snapshot().frontend_until, Some(Moment::from_nanos(100)));
    Ok(())
}

#[test]
fn retired_attempt_cannot_report_readiness_and_successor_starts_without_old_reveal() -> TestResult {
    let mut f = Fixture::new()?;
    authorized(&mut f)?;
    f.accept(4, MainAuthorizationObservation::MainRevealed { identity: f.identity })?;
    f.session.retire(f.identity)?;
    f.rejected(
        5,
        MainAuthorizationObservation::MainRevealed { identity: f.identity },
        rejection(LifecycleRejection::WrongPhase),
    )?;
    f.accept(5, MainAuthorizationObservation::MainRetired { identity: f.identity })?;
    f.session.confirm_retired(f.identity)?;
    let mut next = f.identity;
    next.launch = LaunchId::new(2)?;
    next.navigation = Generation::new(2)?;
    f.session.begin(next, f.lease, Moment::from_nanos(1000))?;
    assert!(!f.session.snapshot().reveal_observed);
    Ok(())
}
