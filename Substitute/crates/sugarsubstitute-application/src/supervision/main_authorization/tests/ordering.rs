// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify original request identities, exact bindings and bounded stage progress.
use super::*;

#[test]
fn actual_request_continuations_authorize_once_in_order() -> TestResult {
    let mut f = Fixture::new()?;
    f.environment()?;
    let env = MainAuthorizationObservation::EnvironmentVerified { identity: f.identity };
    assert_eq!(f.accept(2, env)?, Some(MainAuthorizationEffect::AuthorizeEnvironment(f.identity)));
    assert_eq!(
        f.accept(2, MainAuthorizationObservation::EnvironmentVerified { identity: f.identity })?,
        None
    );
    f.accept(
        2,
        MainAuthorizationObservation::EnvironmentAuthorizationIssued { identity: f.identity },
    )?;
    assert_eq!(
        f.accept(
            3,
            MainAuthorizationObservation::ControllerObserved {
                identity: f.identity,
                browser_process_id: pid()?
            }
        )?,
        Some(MainAuthorizationEffect::ValidateBrowser {
            identity: f.identity,
            browser_process_id: pid()?
        })
    );
    assert_eq!(
        f.accept(
            3,
            MainAuthorizationObservation::BrowserVerified {
                identity: f.identity,
                browser_process_id: pid()?
            }
        )?,
        Some(MainAuthorizationEffect::AuthorizeBrowser {
            identity: f.identity,
            browser_process_id: pid()?
        })
    );
    f.accept(
        3,
        MainAuthorizationObservation::BrowserAuthorizationIssued {
            identity: f.identity,
            browser_process_id: pid()?,
        },
    )?;
    assert_eq!(f.session.stage(), MainAuthorizationStage::Authorized);
    assert_eq!(
        f.accept(
            3,
            MainAuthorizationObservation::BrowserVerified {
                identity: f.identity,
                browser_process_id: pid()?
            }
        )?,
        None
    );
    assert_eq!(f.session.snapshot().request_highwater, 3);
    assert_eq!(f.session.snapshot().frontend_until, Some(Moment::from_nanos(100)));
    Ok(())
}

#[test]
fn controller_and_environment_cannot_skip_real_issue_and_completion() -> TestResult {
    let mut f = Fixture::new()?;
    f.rejected(
        1,
        MainAuthorizationObservation::EnvironmentFolderComplete { identity: f.identity },
        rejection(LifecycleRejection::WrongPhase),
    )?;
    f.environment()?;
    f.rejected(
        2,
        MainAuthorizationObservation::EnvironmentAuthorizationIssued { identity: f.identity },
        rejection(LifecycleRejection::WrongPhase),
    )?;
    f.rejected(
        3,
        MainAuthorizationObservation::ControllerObserved {
            identity: f.identity,
            browser_process_id: pid()?,
        },
        rejection(LifecycleRejection::WrongPhase),
    )?;
    Ok(())
}

#[test]
fn browser_issue_waits_matching_verified_pid() -> TestResult {
    let mut f = Fixture::new()?;
    f.controller()?;
    f.rejected(
        3,
        MainAuthorizationObservation::BrowserAuthorizationIssued {
            identity: f.identity,
            browser_process_id: pid()?,
        },
        rejection(LifecycleRejection::WrongPhase),
    )?;
    let wrong = NonZeroU32::new(72).ok_or_else(|| io::Error::other("positive fixture PID"))?;
    f.rejected(
        3,
        MainAuthorizationObservation::BrowserVerified {
            identity: f.identity,
            browser_process_id: wrong,
        },
        rejection(LifecycleRejection::ChangedDuplicate),
    )?;
    Ok(())
}

#[test]
fn trusted_continuations_cannot_invent_another_request_identity() -> TestResult {
    let mut f = Fixture::new()?;
    f.environment()?;
    f.rejected(
        3,
        MainAuthorizationObservation::EnvironmentVerified { identity: f.identity },
        rejection(LifecycleRejection::StaleRequest),
    )?;
    let mut f = Fixture::new()?;
    f.controller()?;
    f.rejected(
        4,
        MainAuthorizationObservation::BrowserVerified {
            identity: f.identity,
            browser_process_id: pid()?,
        },
        rejection(LifecycleRejection::StaleRequest),
    )?;
    Ok(())
}
