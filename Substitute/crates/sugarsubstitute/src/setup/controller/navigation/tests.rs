// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Native navigation exercises real command/result admission with storage and worker boundaries supplied.
use super::*;
use sugarsubstitute_application::setup_flow::{Operation as Command, Page, RuntimeMode, Status};

fn fixture() -> Result<(Navigation, Controller)> {
    Ok((
        Navigation::new(r"E:\用户\Substitute".into())?,
        Controller::new(PathBuf::new(), PathBuf::new(), &[]),
    ))
}

/// Route real backend results while the fixture supplies only storage/process boundaries.
fn advance(navigation: &mut Navigation, controller: &mut Controller) -> Result<()> {
    let events = controller.advance_events()?.events;
    navigation.advance(controller, events, Duration::ZERO)
}

fn primary(navigation: &mut Navigation, controller: &mut Controller) -> Result<()> {
    if let Some(request) = navigation.flow.primary(Duration::ZERO)? {
        navigation.submit(controller, request)?;
    }
    Ok(())
}

/// Complete work at the storage/worker boundary, preserving the exact admitted command identity.
fn complete(navigation: &mut Navigation, controller: &mut Controller, kind: Kind) -> Result<()> {
    let id = navigation.pending.ok_or("No pending navigation")?.get().to_string();
    controller.waiting = None;
    controller.emit(Some(id), kind)?;
    advance(navigation, controller)
}

fn location(
    navigation: &mut Navigation,
    controller: &mut Controller,
    maintenance: bool,
) -> Result<()> {
    primary(navigation, controller)?;
    advance(navigation, controller)?;
    assert_eq!(navigation.flow.page(), Page::Location);
    primary(navigation, controller)?;
    let root = PathBuf::from(navigation.flow.choices().install_root.clone());
    let existing = maintenance.then(|| {
        Box::new(setup_storage::InstallationSummary {
            selection: requests::selection(sugarsubstitute_application::setup_flow::Choices {
                comfy_root: r"E:\已有环境".into(),
                ..navigation.flow.choices().clone()
            }),
            executable_missing: true,
            launch_entry_missing: true,
        })
    });
    complete(navigation, controller, Kind::Completed(Completed::Location { root, existing }))
}

#[test]
fn native_managed_discovery_preserves_tokens_and_correlates_dependent_results() -> Result<()> {
    let (mut navigation, mut controller) = fixture()?;
    location(&mut navigation, &mut controller, false)?;
    assert_eq!(navigation.flow.page(), Page::Runtime);
    primary(&mut navigation, &mut controller)?;
    assert_eq!(navigation.flow.page(), Page::Configuration);
    for field in ["release", "comfyVersion", "variant"] {
        let waiting = controller.waiting.take().ok_or("Option discovery was not admitted")?;
        assert!(matches!(waiting.command, Command::Options { .. }));
        controller.pending = Some(Pending { id: waiting.id, purpose: Purpose::Options });
        controller.worker_event(json!({"event":"completed", "id":controller.pending.as_ref().ok_or("Pending missing")?.id,
            "data":[{"value":format!("opaque/{field}"),"label":"日本語 🐈", "description":"説明",
                "recommended":true,"selectionToken":if field == "variant" { Some("original-token") } else { None }}]}))?;
        advance(&mut navigation, &mut controller)?;
    }
    assert!(!navigation.flow.busy());
    assert_eq!(navigation.flow.choices().selection_token.as_deref(), Some("original-token"));
    assert!(navigation.flow.can_primary());
    assert!(controller.output.is_empty() && controller.waiting.is_none());
    Ok(())
}

#[test]
fn native_maintenance_open_requires_its_original_completion_and_allows_retry() -> Result<()> {
    let (mut navigation, mut controller) = fixture()?;
    location(&mut navigation, &mut controller, true)?;
    assert_eq!(navigation.flow.page(), Page::Maintenance);
    let existing = navigation.flow.existing().ok_or("Missing inspection")?;
    assert_eq!(existing.comfy_root, r"E:\已有环境");
    assert!(existing.executable_missing && existing.entry_missing);
    // Admit at the application boundary; the external installation work is supplied below.
    let request = navigation.flow.primary(Duration::ZERO)?.ok_or("Maintenance request absent")?;
    navigation.pending = Some(request.id);
    let stale = Event { id: Some("stale".into()), kind: Kind::Completed(Completed::Installed) };
    assert!(navigation.receive(&mut controller, stale, Duration::ZERO).is_err());
    assert!(navigation.flow.busy());
    complete(
        &mut navigation,
        &mut controller,
        Kind::Progress(Progress::storage("Installing".into(), 63.0)),
    )?;
    assert_eq!(navigation.flow.overall(), 63.0);
    complete(&mut navigation, &mut controller, Kind::Completed(Completed::Installed))?;
    assert_eq!(navigation.flow.page(), Page::Complete);
    primary(&mut navigation, &mut controller)?;
    advance(&mut navigation, &mut controller)?;
    assert!(navigation.flow.failure().is_some()); // No fabricated launch authority in this fixture.
    assert!(!navigation.flow.close_requested());
    let request = navigation.flow.primary(Duration::ZERO)?.ok_or("Retry Open absent")?;
    navigation.pending = Some(request.id);
    let id = Some(request.id.get().to_string());
    assert!(
        navigation
            .receive(
                &mut controller,
                Event { id: id.clone(), kind: Kind::Completed(Completed::Installed) },
                Duration::ZERO
            )
            .is_err()
    );
    assert!(!navigation.flow.close_requested());
    navigation.receive(
        &mut controller,
        Event { id, kind: Kind::Completed(Completed::Opened) },
        Duration::ZERO,
    )?;
    assert!(navigation.flow.close_requested());
    Ok(())
}

#[test]
fn native_worker_loss_and_close_do_not_leave_navigation_busy_or_accept_late_results() -> Result<()>
{
    let (mut navigation, mut controller) = fixture()?;
    location(&mut navigation, &mut controller, false)?;
    navigation.flow.set_mode(RuntimeMode::Attached)?;
    primary(&mut navigation, &mut controller)?;
    primary(&mut navigation, &mut controller)?;
    assert!(navigation.flow.busy());
    controller.worker_failed("original worker cause".into())?;
    advance(&mut navigation, &mut controller)?;
    assert!(!navigation.flow.busy());
    assert!(navigation.flow.details_open());
    assert!(!navigation.flow.can_primary());
    assert!(controller.waiting.is_none());
    navigation.flow.closing();
    controller.close()?;
    controller.emit(Some("late".into()), Kind::Completed(Completed::Opened))?;
    advance(&mut navigation, &mut controller)?;
    assert!(matches!(navigation.flow.status(), Status::Cancelling));
    assert!(!navigation.flow.close_requested());
    assert!(controller.output.is_empty());
    Ok(())
}

#[test]
fn worker_progress_retains_phase_and_original_identity_for_native_navigation() -> Result<()> {
    let (_, mut controller) = fixture()?;
    controller.pending = Some(Pending { id: "original".into(), purpose: Purpose::Integrate });
    controller.worker_event(json!({"id":"original","event":"progress","data":{
        "phase":"integration", "percent":50, "statusCode":"checkingSugarCubes"
    }}))?;
    let event = controller.output.pop_front().ok_or("No progress")?;
    assert_eq!(event.id.as_deref(), Some("original"));
    let Kind::Progress(progress) = event.kind else { return Err("Wrong event".into()) };
    assert_eq!(progress.phase.as_deref(), Some("integration"));
    assert_eq!(progress.percent, Some(50.0));
    assert_eq!(progress.overall, 96.5);
    assert_eq!(
        progress.status,
        sugarsubstitute_contracts::localization::installer::messages("en").checking_sugar_cubes
    );
    assert!(controller.worker_event(json!({"id":"other","event":"completed","data":{}})).is_err());
    assert_eq!(controller.pending.as_ref().ok_or("Pending consumed")?.id, "original");
    Ok(())
}

#[test]
fn worker_diagnostics_are_projected_without_mutating_data_or_consuming_work() -> Result<()> {
    let (_, mut controller) = fixture()?;
    controller.pending = Some(Pending { id: "original".into(), purpose: Purpose::Options });
    for data in [
        json!({"level":"warn","message":"original diagnostic 🐈"}),
        json!({"event":"download","properties":{"cached":true,"bytes":8192}}),
    ] {
        controller.worker_event(json!({"event":"diagnostic","id":"original","data":data}))?;
        let event = controller.output.pop_front().ok_or("Diagnostic was discarded")?;
        let Kind::Diagnostic(diagnostic) = event.kind else {
            return Err("Wrong diagnostic event".into());
        };
        assert_eq!(serde_json::to_value(diagnostic)?, crate::diagnostics::worker::project(&data));
        assert_eq!(controller.pending.as_ref().ok_or("Request consumed")?.id, "original");
    }
    assert!(!controller.unavailable);
    Ok(())
}

/// Cancellation details survive the worker boundary and reach the native failure presentation.
#[test]
fn native_cancellation_preserves_original_detail_or_localized_fallback() -> Result<()> {
    use sugarsubstitute_application::setup_flow::Failure;
    for cause in [None, Some("Cancelled by upstream with original detail 🐈".to_owned())] {
        let (mut navigation, mut controller) = fixture()?;
        primary(&mut navigation, &mut controller)?;
        advance(&mut navigation, &mut controller)?;
        primary(&mut navigation, &mut controller)?;
        complete(&mut navigation, &mut controller, Kind::Cancelled(cause.clone()))?;
        assert!(!navigation.flow.busy());
        match (cause, navigation.flow.failure()) {
            (Some(expected), Some(Failure::Message(actual))) => assert_eq!(&expected, actual),
            (
                None,
                Some(Failure::Localized(
                    sugarsubstitute_contracts::localization::installer::InstallerText::Cancelled,
                )),
            ) => {}
            _ => return Err("Native cancellation lost its original cause".into()),
        }
    }
    Ok(())
}
