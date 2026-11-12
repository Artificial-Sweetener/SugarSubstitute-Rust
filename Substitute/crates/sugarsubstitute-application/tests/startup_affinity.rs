// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve owner-thread diagnostics through the real acknowledged startup session.
use std::{cell::Cell, error::Error, fmt, rc::Rc};
use sugarsubstitute_application::startup::{
    SessionFailure, StartupPhase, StartupRejection, StartupSession,
};
use sugarsubstitute_contracts::{
    configuration::{
        ConnectionConfiguration, LoopbackOrigin, Presentation, ProfileId, StartupConfiguration,
        Theme,
    },
    execution::Generation,
    startup::{
        ConnectionCleanup, LaunchId, StartupEffect, StartupEvent, StartupFailure,
        StartupFailureCode, StartupObservation,
    },
};

#[derive(Debug)]
struct UiDiagnostic {
    released: Rc<Cell<usize>>,
    detail: &'static str,
}
impl fmt::Display for UiDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.detail)
    }
}
impl Error for UiDiagnostic {}
impl Drop for UiDiagnostic {
    fn drop(&mut self) {
        self.released.set(self.released.get() + 1);
    }
}
fn configuration() -> Result<StartupConfiguration, Box<dyn Error>> {
    Ok(StartupConfiguration::new(
        ConnectionConfiguration::Attach(LoopbackOrigin::parse("http://127.0.0.1:8188/", 8188)?),
        ProfileId::new("isolated-owner".into())?,
        Presentation::new("en", Theme::Dark, false)?,
    ))
}
fn failure(
    launch: u64,
    released: &Rc<Cell<usize>>,
    detail: &'static str,
) -> Result<StartupEvent, Box<dyn Error>> {
    Ok(StartupEvent::new(
        LaunchId::new(launch)?,
        StartupObservation::Failed(StartupFailure::new(
            StartupFailureCode::Presentation,
            Box::new(UiDiagnostic { released: released.clone(), detail }),
        )),
    ))
}

#[test]
fn ui_affine_cause_remains_owned_through_foreign_rejection_settlement_and_retry()
-> Result<(), Box<dyn Error>> {
    let mut session = StartupSession::new(LaunchId::new(1)?, Generation::new(1)?, configuration()?);
    let foreign_drops = Rc::new(Cell::new(0));
    let rejected = session
        .observe(failure(2, &foreign_drops, "Foreign UI diagnostic")?)
        .err()
        .ok_or("Foreign failure admitted")?;
    assert_eq!(rejected.reason(), StartupRejection::ForeignLaunch);
    assert_eq!(session.phase(), StartupPhase::SplashFirstFrame);
    assert!(session.failure().is_none());
    assert_eq!(foreign_drops.get(), 0);
    let StartupObservation::Failed(retained) = rejected.into_event().into_observation() else {
        return Err("Rejected original failure lost".into());
    };
    let diagnostic = retained
        .source()
        .and_then(|cause| cause.downcast_ref::<UiDiagnostic>())
        .ok_or("Original non-Send diagnostic lost")?;
    assert_eq!(diagnostic.detail, "Foreign UI diagnostic");
    drop(retained);
    assert_eq!(foreign_drops.get(), 1);

    let accepted_drops = Rc::new(Cell::new(0));
    assert_eq!(
        session.observe(failure(1, &accepted_drops, "Native initialization failed")?)?,
        Some(StartupEffect::CleanupConnection {
            launch: LaunchId::new(1)?,
            mode: ConnectionCleanup::DisconnectAttachment
        })
    );
    assert_eq!(session.phase(), StartupPhase::Failed);
    session.observe(StartupEvent::new(LaunchId::new(1)?, StartupObservation::ConnectionSettled))?;
    assert_eq!(session.phase(), StartupPhase::Stopped);
    let Some(SessionFailure::Adapter(retained)) = session.failure() else {
        return Err("Accepted native cause lost at settlement".into());
    };
    assert_eq!(
        retained
            .source()
            .and_then(|cause| cause.downcast_ref::<UiDiagnostic>())
            .ok_or("Accepted original type lost")?
            .detail,
        "Native initialization failed"
    );
    assert_eq!(accepted_drops.get(), 0);
    session.retry(LaunchId::new(2)?, Generation::new(1)?, configuration()?)?;
    assert_eq!(accepted_drops.get(), 1);
    assert!(session.failure().is_none());
    Ok(())
}
