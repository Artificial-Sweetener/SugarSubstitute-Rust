// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep queued/current roles and original budgets independent from completed historical roles.
use super::*;
use sugarsubstitute_application::supervision::DesktopExitTransaction;
use sugarsubstitute_contracts::{execution::Moment, lifecycle::*};
fn actual_request(instance: u64, at: u64) -> Result<DesktopCloseRequest> {
    let owner = CoordinatorKey {
        namespace: NamespaceId::new([1; 32]),
        epoch: CoordinatorEpoch::new([2; 16])?,
    };
    let run =
        RunKey { owner, instance: InstanceId::new(instance)?, generation: RunGeneration::new(1)? };
    let mut exit =
        DesktopExitTransaction::new(run, ProcessKey { owner, id: ProcessId::new(instance)? })?;
    exit.begin(
        run,
        ShutdownId::new(instance)?,
        CompletionIntent::Normal,
        0,
        Moment::from_nanos(at),
    )?;
    exit.normal_close_request().ok_or_else(|| "Actual deliberate request missing".into())
}
#[test]
fn normal_idle_two_roles_keep_original_unresolved_cutoff_and_deny_live_or_queued_other_role()
-> Result<()> {
    let first = actual_request(1, 10)?.budget();
    let second = actual_request(2, 30)?.budget();
    assert_eq!(
        original_budget(
            [
                Some(Role::Normal { budget: first, retired: false }),
                Some(Role::Normal { budget: second, retired: false })
            ]
            .into_iter(),
            None
        ),
        Some(first)
    );
    assert_eq!(
        original_budget(
            [Some(Role::Normal { budget: first, retired: true }), None].into_iter(),
            None
        ),
        None
    );
    assert_eq!(
        original_budget(
            [Some(Role::Normal { budget: first, retired: false }), None].into_iter(),
            None
        ),
        None
    );
    // A native-retired role still awaiting policy settlement remains unresolved.
    assert_eq!(
        original_budget(
            [
                Some(Role::Normal { budget: first, retired: false }),
                Some(Role::Normal { budget: second, retired: true })
            ]
            .into_iter(),
            None
        ),
        Some(first)
    );
    // Completed is the caller's existing sealed retirement disposition, never a new proof from this fold.
    assert_eq!(
        original_budget(
            [
                Some(Role::Normal { budget: first, retired: true }),
                Some(Role::Normal { budget: second, retired: false })
            ]
            .into_iter(),
            None
        ),
        Some(second)
    );
    assert_eq!(
        original_budget(
            [
                Some(Role::Normal { budget: first, retired: true }),
                Some(Role::Normal { budget: second, retired: true })
            ]
            .into_iter(),
            None
        ),
        Some(second)
    );
    assert_eq!(
        original_budget(
            [
                Some(Role::Normal { budget: second, retired: true }),
                Some(Role::Normal { budget: first, retired: true })
            ]
            .into_iter(),
            None
        ),
        Some(second)
    );
    Ok(())
}
#[test]
fn normal_idle_empty_queued_projection_and_retained_failure_never_issue_normal_close() -> Result<()>
{
    let mut owner = ProductionActivation::new(
        r"E:\application\SugarSubstitute.exe".into(),
        r"E:\application".into(),
        r"D:\captures".into(),
    )?;
    assert!(!owner.normal_close_requested());
    assert!(owner.shutdown_budget().is_none());
    let request = actual_request(1, 10)?;
    owner.pending[0] = Some(PendingProjection {
        restart: false,
        qualification: Default::default(),
        run: request.run(),
        launch: LaunchId::new(1)?,
        navigation: Generation::new(1)?,
        connection: ConnectionId::new(1)?,
        job: None,
        completion: None,
    });
    assert!(!owner.normal_close_requested());
    assert!(owner.shutdown_budget().is_none());
    owner.pending[0] = None;
    owner.preserve(Box::new(std::io::Error::other("Original activation failure")));
    assert!(!owner.normal_close_requested());
    assert!(owner.failure_source().is_some());
    Ok(())
}
#[test]
fn normal_idle_partial_unlaunched_or_failed_record_never_becomes_a_normal_role() -> Result<()> {
    let request = actual_request(1, 10)?;
    let record = Record {
        qualification: Default::default(),
        run: request.run(),
        launch: LaunchId::new(1)?,
        navigation: Generation::new(1)?,
        connection: ConnectionId::new(1)?,
        projected: None,
        attach: None,
        attached_memory: None,
        attach_failure: None,
        released_attach: None,
        attach_retirement: None,
        start: None,
        parent: None,
        retained_causes: None,
        preparation: None,
        retired: None,
        forwarded: None,
        ui_suspected: false,
        replied: true,
        closed: false,
        seed: incident_seed::SeedPreparation::new(),
        seeded: false,
        collection: incident_collection::IncidentCollection::new(),
        failed: None,
        failed_cause: None,
        failed_shutdown: None,
        failed_family_adopted: false,
        pending_desktop_failure: None,
        restart: restart::RestartState::new(),
    };
    assert!(ProductionActivation::normal_role_budget(&record).is_none());
    assert!(
        ProductionActivation::overdue_role_budget(&record, Moment::from_nanos(u64::MAX)).is_none()
    );
    assert!(role(&record).is_none());
    Ok(())
}

#[test]
fn normal_idle_capture_query_preserves_actual_dormant_registration_and_denies_foreign_owner()
-> Result<()> {
    let request = actual_request(1, 10)?;
    let capture = CaptureRegistration::new(request.run().owner);
    let before = capture.capture.snapshot();
    assert!(capture.normal_close_pending(request.run().owner));
    let foreign = CoordinatorKey { epoch: CoordinatorEpoch::new([7; 16])?, ..request.run().owner };
    assert!(capture.normal_close_pending(foreign));
    let after = capture.capture.snapshot();
    assert_eq!(before.owner, after.owner);
    assert_eq!(before.state, after.state);
    assert_eq!(after.state, sugarsubstitute_contracts::crash::CaptureState::Dormant);
    assert_eq!(before.handler, after.handler);
    assert!(after.handler.is_none());
    Ok(())
}

#[test]
fn normal_idle_checked_failed_history_requires_real_later_close_intent_and_denies_live_peer()
-> Result<()> {
    let original = actual_request(1, 10)?.budget();
    let dismissed = actual_request(2, 30)?.budget();
    assert_eq!(original_budget([Some(Role::SettledFailedDesktop)].into_iter(), None), None);
    assert_eq!(
        original_budget([Some(Role::SettledFailedDesktop)].into_iter(), Some(dismissed)),
        Some(dismissed)
    );
    assert_eq!(
        original_budget([Some(Role::SettledFailedDesktop), None].into_iter(), Some(dismissed)),
        None
    );
    assert_eq!(
        original_budget(
            [Some(Role::Normal { budget: original, retired: false })].into_iter(),
            Some(dismissed)
        ),
        Some(original)
    );
    assert_eq!(
        original_budget(
            [Some(Role::Normal { budget: original, retired: true })].into_iter(),
            Some(dismissed)
        ),
        Some(dismissed)
    );
    Ok(())
}
