// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Execute original startup and reload through the same actual application session.
use super::*;
fn running() -> Result<StartupSession, Box<dyn Error>> {
    let mut owner = session(false)?;
    loading(&mut owner)?;
    owner.observe(event(StartupObservation::FrontendReceipt(receipt(4, 7, false)?))?)?;
    owner.observe(event(StartupObservation::SplashRetired)?)?;
    owner.observe(event(StartupObservation::MainRevealed)?)?;
    Ok(owner)
}
#[test]
fn reload_preserves_backend_origin_and_requires_new_receipt_then_actual_reveal() -> TestResult {
    let mut owner = running()?;
    let configuration = owner.configuration().clone();
    owner.reload_frontend(LaunchId::new(5)?, Generation::new(8)?)?;
    assert_eq!(owner.configuration(), &configuration);
    assert_eq!(owner.connection_origin(), Some(origin()?));
    // Use explicit typed events so no fake splash completion is supplied on reload.
    assert!(matches!(
        owner.observe(StartupEvent::new(
            LaunchId::new(5)?,
            StartupObservation::ConnectionReady(origin()?)
        ))?,
        Some(StartupEffect::LoadFrontend { .. })
    ));
    owner.observe(StartupEvent::new(
        LaunchId::new(5)?,
        StartupObservation::FrontendNavigationStarted(Generation::new(8)?),
    ))?;
    assert!(
        owner.observe(event(StartupObservation::FrontendReceipt(receipt(4, 7, false)?))?).is_err()
    );
    let effect = owner.observe(StartupEvent::new(
        LaunchId::new(5)?,
        StartupObservation::FrontendReceipt(receipt(5, 8, false)?),
    ))?;
    assert!(matches!(effect, Some(StartupEffect::RevealMain { .. })));
    assert_eq!(owner.phase(), StartupPhase::RevealingMain);
    owner.observe(StartupEvent::new(LaunchId::new(5)?, StartupObservation::MainRevealed))?;
    assert_eq!(owner.phase(), StartupPhase::Running);
    Ok(())
}
#[test]
fn reload_rejects_unready_or_reused_identity_without_changing_live_configuration() -> TestResult {
    let mut unready = session(false)?;
    assert!(unready.reload_frontend(LaunchId::new(5)?, Generation::new(8)?).is_err());
    let mut owner = running()?;
    for (launch, nav) in [(4, 8), (5, 7)] {
        assert!(owner.reload_frontend(LaunchId::new(launch)?, Generation::new(nav)?).is_err());
        assert_eq!(owner.phase(), StartupPhase::Running);
        assert_eq!(owner.launch().get(), 4);
    }
    Ok(())
}
#[test]
fn cancelled_reload_rejects_readiness_and_wrong_origin_cannot_replace_connection() -> TestResult {
    let mut owner = running()?;
    owner.reload_frontend(LaunchId::new(5)?, Generation::new(8)?)?;
    let wrong = LoopbackOrigin::parse("http://127.0.0.1:8188", 8188)?;
    assert!(
        owner
            .observe(StartupEvent::new(
                LaunchId::new(5)?,
                StartupObservation::ConnectionReady(wrong)
            ))
            .is_err()
    );
    assert_eq!(owner.connection_origin(), Some(origin()?));
    owner.intent(LaunchId::new(5)?, SplashIntent::Cancel)?;
    assert!(
        owner
            .observe(StartupEvent::new(
                LaunchId::new(5)?,
                StartupObservation::ConnectionReady(origin()?)
            ))
            .is_err()
    );
    Ok(())
}
