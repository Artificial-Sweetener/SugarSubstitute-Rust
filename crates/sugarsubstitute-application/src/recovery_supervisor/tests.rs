// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise original ACK/retirement ordering, immutable deadlines and separate process identities.
use super::*;
use sugarsubstitute_contracts::{incident::*, lifecycle::*};
type TestResult = Result<(), Box<dyn std::error::Error>>;
fn facts()
-> Result<(RecoveryGrantKey, DurableAcknowledgement, EntrySetupBudget), Box<dyn std::error::Error>>
{
    let owner = CoordinatorKey {
        namespace: NamespaceId::new([1; 32]),
        epoch: CoordinatorEpoch::new([2; 16])?,
    };
    let run = RunKey { owner, instance: InstanceId::new(1)?, generation: RunGeneration::new(1)? };
    let binding = ReportBinding::new(
        IncidentKey::new(IncidentSubject::Desktop(run)),
        IncidentRevision::new(2)?,
        ReportSessionId::new(1)?,
    );
    let key = RecoveryGrantKey::new(RecoveryGrantId::new(1)?, run, binding)?;
    let ack = DurableAcknowledgement::new(
        binding.incident(),
        binding.revision().next()?,
        IncidentUtc::from_unix_seconds(100)?,
    );
    Ok((key, ack, EntrySetupBudget::new(Moment::from_nanos(17))?))
}
fn launched() -> Result<(RecoverySupervisor, RecoveryGrantKey), Box<dyn std::error::Error>> {
    let (key, ack, budget) = facts()?;
    let mut owner = RecoverySupervisor::new(key, budget);
    owner.admitted(key, budget.started())?;
    owner.acknowledged(key, ack, budget.started())?;
    owner.original_family_retired(key.previous())?;
    owner.original_profile_retired(key.previous())?;
    assert!(matches!(owner.take_effect(), Some(RecoverySupervisorEffect::Launch { .. })));
    owner.launch_issued(key, budget.started())?;
    Ok((owner, key))
}
#[test]
fn every_ack_family_profile_order_produces_one_admitted_successor_without_changing_owner()
-> TestResult {
    for order in [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]] {
        let (key, ack, budget) = facts()?;
        let mut owner = RecoverySupervisor::new(key, budget);
        assert!(!owner.acknowledgement_allowed(key, budget.started()));
        assert_eq!(owner.acknowledged(key, ack, budget.started()), Err(RecoveryRejection::Phase));
        owner.admitted(key, budget.started())?;
        for (position, step) in order.into_iter().enumerate() {
            match step {
                0 => owner.acknowledged(key, ack, budget.started())?,
                1 => owner.original_family_retired(key.previous())?,
                _ => owner.original_profile_retired(key.previous())?,
            }
            if position != 2 {
                assert!(owner.take_effect().is_none());
            }
        }
        assert_eq!(
            owner.take_effect(),
            Some(RecoverySupervisorEffect::Launch { key, original: budget })
        );
        assert!(owner.take_effect().is_none());
        owner.launch_issued(key, budget.started())?;
        assert_eq!(owner.launch_issued(key, budget.started()), Err(RecoveryRejection::Phase));
        assert_eq!(key.run().owner, key.previous().owner);
        assert_eq!(key.run().instance, key.previous().instance);
        assert_eq!(key.run().generation, key.previous().generation.next()?);
    }
    Ok(())
}
#[test]
fn foreign_changed_or_expired_restart_never_admits_ack_or_launch() -> TestResult {
    let (key, ack, budget) = facts()?;
    let foreign = RecoveryGrantKey::new(RecoveryGrantId::new(2)?, key.previous(), key.report())?;
    let mut owner = RecoverySupervisor::new(key, budget);
    assert_eq!(owner.admitted(foreign, budget.started()), Err(RecoveryRejection::Binding));
    assert_eq!(owner.admitted(key, budget.until()), Err(RecoveryRejection::Deadline));
    owner.admitted(key, budget.started())?;
    assert!(!owner.acknowledgement_allowed(foreign, budget.started()));
    assert!(!owner.acknowledgement_allowed(key, budget.until()));
    let wrong = DurableAcknowledgement::new(ack.incident(), ack.revision().next()?, ack.at());
    assert_eq!(owner.acknowledged(key, wrong, budget.started()), Err(RecoveryRejection::Binding));
    assert_eq!(owner.original_family_retired(key.run()), Err(RecoveryRejection::Binding));
    assert!(owner.take_effect().is_none());
    owner.acknowledged(key, ack, budget.started())?;
    owner.acknowledged(key, ack, budget.started())?;
    assert!(owner.take_effect().is_none());
    Ok(())
}
#[test]
fn fresh_supervisor_capture_cannot_stand_in_for_actual_desktop_capture_or_old_client_repair()
-> TestResult {
    let (mut owner, key) = launched()?;
    let now = Moment::from_nanos(17);
    let supervisor = RecoveryProcessIdentity::new(101, 201)?;
    let handler = RecoveryProcessIdentity::new(102, 202)?;
    let desktop = RecoveryProcessIdentity::new(103, 203)?;
    owner.authenticated(key, supervisor, now)?;
    assert_eq!(owner.snapshot().coverage, RecoveryCaptureCoverage::PriorRegistrationsLost);
    assert_eq!(
        owner.capture_registered(key, supervisor, supervisor, now),
        Err(RecoveryRejection::Binding)
    );
    owner.capture_registered(key, supervisor, handler, now)?;
    assert_eq!(
        owner.snapshot().coverage,
        RecoveryCaptureCoverage::FreshSupervisorRegistered { supervisor, handler }
    );
    assert_eq!(
        owner.desktop_capture_registered(key, supervisor, now),
        Err(RecoveryRejection::Binding)
    );
    assert_eq!(
        owner.desktop_capture_registered(key, handler, now),
        Err(RecoveryRejection::Binding)
    );
    assert_eq!(owner.snapshot().desktop, None);
    owner.desktop_capture_registered(key, desktop, now)?;
    let snapshot = owner.snapshot();
    assert_eq!(snapshot.desktop, Some(desktop));
    assert_eq!(snapshot.supervisor, Some(supervisor));
    assert_eq!(
        snapshot.coverage,
        RecoveryCaptureCoverage::FreshFamilyRegistered { supervisor, handler }
    );
    assert_eq!(snapshot.phase, RecoverySupervisorPhase::Active);
    Ok(())
}
#[test]
fn cancelled_or_timed_out_admission_requires_all_actual_original_retirements() -> TestResult {
    for reason in [RecoveryFailureCode::Cancelled, RecoveryFailureCode::Deadline] {
        let (key, _, original) = facts()?;
        let mut owner = RecoverySupervisor::new(key, original);
        owner.admitted(key, original.started())?;
        owner.fail(reason);
        owner.fail(RecoveryFailureCode::Transport);
        assert_eq!(owner.snapshot().first_failure, Some(reason));
        assert!(!owner.acknowledgement_allowed(key, original.started()));
        assert_eq!(owner.take_effect(), Some(RecoverySupervisorEffect::BeginShutdown { key }));
        assert!(owner.take_effect().is_none());
        let shutdown = ShutdownBudget::new(original.started())?;
        owner.shutdown_admitted(shutdown)?;
        assert_eq!(
            owner.shutdown_admitted(ShutdownBudget::new(Moment::from_nanos(18))?),
            Err(RecoveryRejection::Deadline)
        );
        for (index, proof) in [
            RecoveryRetirement::Supervisor,
            RecoveryRetirement::DesktopFamily,
            RecoveryRetirement::Transport,
            RecoveryRetirement::Capture,
        ]
        .into_iter()
        .enumerate()
        {
            owner.retired(key, proof)?;
            if index != 3 {
                assert_eq!(owner.snapshot().phase, RecoverySupervisorPhase::Retiring);
            }
        }
        assert_eq!(owner.snapshot().phase, RecoverySupervisorPhase::Retired);
        assert_eq!(owner.snapshot().first_failure, Some(reason));
    }
    Ok(())
}
#[test]
fn original_handshake_expiry_closes_admission_and_does_not_renew_shutdown() -> TestResult {
    let (mut owner, key) = launched()?;
    let original = EntrySetupBudget::new(Moment::from_nanos(17))?;
    owner.observe_time(original.until());
    assert_eq!(owner.snapshot().first_failure, Some(RecoveryFailureCode::Deadline));
    assert_eq!(
        owner.authenticated(key, RecoveryProcessIdentity::new(101, 201)?, original.until()),
        Err(RecoveryRejection::Deadline)
    );
    assert!(matches!(owner.take_effect(), Some(RecoverySupervisorEffect::BeginShutdown { .. })));
    let shutdown = ShutdownBudget::new(original.until())?;
    owner.shutdown_admitted(shutdown)?;
    owner.observe_time(shutdown.settle_until);
    assert!(owner.take_effect().is_none());
    assert_eq!(owner.snapshot().phase, RecoverySupervisorPhase::Retiring);
    Ok(())
}
