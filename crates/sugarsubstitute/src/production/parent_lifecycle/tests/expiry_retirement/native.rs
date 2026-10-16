// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit only a genuine current native negative fact and its real checked ACK.
use super::*;
use sugarsubstitute_application::supervision::{
    MainAuthorizationObservation, MainAuthorizationStage,
};

#[test]
fn genuine_current_negative_fact_needs_actual_ack_before_expired_attempt_cleanup() -> Result<()> {
    let (mut parent, h, lease) = expired()?;
    let retired = retirement_header(h)?;
    assert!(
        parent
            .observe_native_retirement(copied_binding(lease), identity()?, retired, false)
            .map_err(|e| e as Box<dyn Error>)?
    );
    assert_eq!(parent.main.stage(), MainAuthorizationStage::Retired);
    assert_eq!(parent.main.snapshot().identity, None);
    let acquisition = parent.acquisition.as_ref().ok_or("Missing")?;
    assert!(!acquisition.retirement_written());
    assert!(
        acquisition
            .settlement_lease(
                copied_binding(lease),
                identity()?,
                h.request.ok_or("Origin")?,
                None,
                None
            )
            .is_err()
    );
    acknowledge_retirement(&mut parent, retired)?;
    assert_eq!(
        parent.acquisition.as_ref().ok_or("Missing")?.settlement_lease(
            copied_binding(lease),
            identity()?,
            h.request.ok_or("Origin")?,
            None,
            None
        )?,
        Some(lease)
    );
    assert!(!parent.native_retired());
    Ok(())
}

#[test]
fn negative_fact_rejects_foreign_or_pending_and_current_main_keeps_positive_path() -> Result<()> {
    for fault in 0..4 {
        let (mut parent, h, lease) = expired()?;
        let mut id = identity()?;
        let mut binding = copied_binding(lease);
        let mut pending = false;
        match fault {
            0 => id.launch = LaunchId::new(2)?,
            1 => binding.connection = ConnectionId::new(2)?,
            2 => pending = true,
            _ => parent.main.begin(id, lease, Moment::from_nanos(200_000_000_000))?,
        }
        let result = parent.observe_native_retirement(binding, id, retirement_header(h)?, pending);
        if fault == 3 {
            assert!(!result.map_err(|e| e as Box<dyn Error>)?);
            assert_eq!(parent.main.stage(), MainAuthorizationStage::RequestedFolder);
        } else {
            assert!(result.is_err());
            assert!(!parent.acquisition.as_ref().ok_or("Missing")?.retirement_written());
        }
    }
    Ok(())
}

#[test]
fn checked_retired_predecessor_is_scoped_to_older_same_run_launch() -> Result<()> {
    let (mut parent, h, lease) = expired()?;
    use sugarsubstitute_application::startup::StartupSession;
    use sugarsubstitute_contracts::{
        configuration::{
            ConnectionConfiguration, LoopbackOrigin, Presentation, ProfileId, StartupConfiguration,
            Theme,
        },
        startup::{StartupEvent, StartupFailure, StartupFailureCode, StartupObservation},
    };
    let previous = MainIdentity { navigation: Generation::new(91)?, ..identity()? };
    parent.main.begin(previous, lease, Moment::from_nanos(30_000_000_000))?;
    parent.main.retire(previous)?;
    parent.main.accept(
        copied_binding(lease),
        RequestId::new(8)?,
        MainAuthorizationObservation::MainRetired { identity: previous },
        Moment::from_nanos(31_000_000_000),
    )?;
    parent.main.confirm_retired(previous)?;
    let configuration = StartupConfiguration::new(
        ConnectionConfiguration::Attach(LoopbackOrigin::parse("http://127.0.0.1:8188", 8188)?),
        ProfileId::new("RetryRetirement".into())?,
        Presentation::new("en", Theme::Dark, true)?,
    );
    let mut session = StartupSession::new(previous.launch, previous.navigation, configuration);
    session.observe(StartupEvent::new(previous.launch, StartupObservation::SplashPresented))?;
    session.observe(StartupEvent::new(
        previous.launch,
        StartupObservation::Failed(StartupFailure::new(
            StartupFailureCode::ConnectionUnavailable,
            Box::new(std::io::Error::other("original failed attempt")),
        )),
    ))?;
    session.observe(StartupEvent::new(previous.launch, StartupObservation::ConnectionSettled))?;
    session.retry_configured(LaunchId::new(2)?, Generation::new(1)?)?;
    let current =
        MainIdentity { launch: session.launch(), navigation: session.navigation(), ..previous };
    let next = WireHeader { request: Some(RequestId::new(9)?), ..h };
    let mut acquisition = super::super::super::acquisition::Acquisition::new(
        copied_binding(lease),
        next,
        current,
        Moment::from_nanos(180_000_000_000),
        sugarsubstitute_application::startup::ConnectionBudgetKind::Attached,
    )?;
    acquisition.observe_backend(next, RequestOutcome::Pending(lease))?;
    acquisition.progress_queued(next)?;
    acquisition.progress_finished(next, true)?;
    parent.acquisition = Some(acquisition);
    parent
        .expire_acquisition(Moment::from_nanos(180_000_000_000))
        .map_err(|e| e as Box<dyn Error>)?;
    let retired =
        WireHeader { request: Some(RequestId::new(10)?), deadline_qpc: 195_000_000_000, ..next };
    assert!(
        parent
            .observe_native_retirement(copied_binding(lease), current, retired, false)
            .map_err(|e| e as Box<dyn Error>)?
    );
    acknowledge_retirement(&mut parent, retired)?;
    assert_eq!(parent.main.snapshot().identity, Some(previous));
    assert_eq!(
        parent.acquisition.as_ref().ok_or("Missing")?.settlement_lease(
            copied_binding(lease),
            current,
            RequestId::new(9)?,
            None,
            Some(previous)
        )?,
        Some(lease)
    );
    Ok(())
}
