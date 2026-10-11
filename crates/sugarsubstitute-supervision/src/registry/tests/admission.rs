// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify capacity, foreign/stale keys and pre-effect admission using actual execution ports.
use super::*;

#[test]
fn foreign_owner_and_invalid_job_order_have_no_native_or_scope_effect() -> TestResult {
    let mut h = Harness::new()?;
    let watch = h.spec(JobClass::NativeObservation, 4096)?;
    let input = input()?;
    let create = h.spec(JobClass::InfrastructureLifecycle, input.required_reservation_bytes())?;
    let FamilyBinding::Desktop(mut run) = binding(1)? else {
        return Err("Invalid fixture".into());
    };
    run.owner.epoch = CoordinatorEpoch::new([2; 16])?;
    let native = ScopeSpec { id: ScopeId::new(2)?, deadline: None };
    assert!(matches!(
        h.registry.submit_launch(
            &h.client,
            &h.scope,
            native,
            create,
            watch,
            FamilyBinding::Desktop(run),
            input
        ),
        Err(RegistryAdmissionError::ForeignOwner)
    ));
    let input = super::input()?;
    assert!(matches!(
        h.registry.submit_launch(&h.client, &h.scope, native, watch, create, binding(1)?, input),
        Err(RegistryAdmissionError::InvalidSpecification)
    ));
    let still_fresh = h.client.create_scope(native, None)?;
    assert_eq!(still_fresh.id().get(), 2);
    assert_eq!(h.fake.creates.load(Ordering::Acquire), 0);
    assert!(h.registry.is_empty());
    Ok(())
}

#[test]
fn memory_undersizing_and_native_deadlines_reject_before_watch_or_creation() -> TestResult {
    let mut h = Harness::new()?;
    let watch = h.spec(JobClass::NativeObservation, 4096)?;
    let input = input()?;
    let create =
        h.spec(JobClass::InfrastructureLifecycle, input.required_reservation_bytes() - 1)?;
    assert!(matches!(
        h.registry.submit_launch(
            &h.client,
            &h.scope,
            ScopeSpec { id: ScopeId::new(2)?, deadline: None },
            create,
            watch,
            binding(1)?,
            input
        ),
        Err(RegistryAdmissionError::InvalidSpecification)
    ));
    let input = super::input()?;
    let create = h.spec(JobClass::InfrastructureLifecycle, input.required_reservation_bytes())?;
    assert!(matches!(
        h.registry.submit_launch(
            &h.client,
            &h.scope,
            ScopeSpec { id: ScopeId::new(2)?, deadline: Some(Moment::from_nanos(1)) },
            create,
            watch,
            binding(1)?,
            input
        ),
        Err(RegistryAdmissionError::InvalidSpecification)
    ));
    assert_eq!(h.fake.creates.load(Ordering::Acquire), 0);
    assert_eq!(h.scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn rejected_creation_retains_actual_unbound_retirement_then_releases_private_slot() -> TestResult {
    let mut h = Harness::new()?;
    h.scope.cancel();
    assert!(matches!(
        h.launch(1)
            .err()
            .and_then(|error| error.downcast::<RegistryAdmissionError>().ok())
            .as_deref(),
        Some(RegistryAdmissionError::Execution(_))
    ));
    h.drain_until(WindowsProcessRegistry::is_empty)?;
    assert_eq!(h.fake.creates.load(Ordering::Acquire), 0);
    assert_eq!(h.next_job, 3);
    Ok(())
}

#[test]
fn current_or_changed_generation_cannot_overlap_retained_owned_family() -> TestResult {
    let mut h = Harness::new()?;
    let ticket = h.launch(1)?;
    h.created(ticket.process())?;
    let input = input()?;
    let watch = h.spec(JobClass::NativeObservation, 4096)?;
    let create = h.spec(JobClass::InfrastructureLifecycle, input.required_reservation_bytes())?;
    let FamilyBinding::Desktop(mut run) = binding(1)? else {
        return Err("Invalid fixture".into());
    };
    run.generation = RunGeneration::new(2)?;
    assert!(matches!(
        h.registry.submit_launch(
            &h.client,
            &h.scope,
            ScopeSpec { id: ScopeId::new(3)?, deadline: None },
            create,
            watch,
            FamilyBinding::Desktop(run),
            input
        ),
        Err(RegistryAdmissionError::Busy)
    ));
    assert_eq!(h.fake.creates.load(Ordering::Acquire), 1);
    Ok(())
}

#[test]
fn sixty_four_desktop_slots_include_uncertain_records_without_displacing_other_kind() -> TestResult
{
    let mut h = Harness::new()?;
    for instance in 1..=64 {
        let ticket = h.launch(instance)?;
        h.created(ticket.process())?;
    }
    assert!(h.launch(65).is_err());
    assert_eq!(h.registry.records.iter().flatten().count(), 64);
    assert_eq!(h.fake.creates.load(Ordering::Acquire), 64);
    Ok(())
}

#[test]
fn immutable_stop_budget_and_exact_key_reject_foreign_stale_or_changed_transactions() -> TestResult
{
    let mut h = Harness::new()?;
    let ticket = h.launch(1)?;
    h.created(ticket.process())?;
    let key = ticket.process();
    let budget = ShutdownBudget::new(Moment::from_nanos(0))?;
    h.registry.begin_stop(key, ShutdownId::new(1)?, budget)?;
    h.registry.begin_stop(key, ShutdownId::new(1)?, budget)?;
    let mut altered = budget;
    altered.graceful_until = Moment::from_nanos(1);
    assert!(matches!(
        h.registry.begin_stop(key, ShutdownId::new(1)?, altered),
        Err(RegistryAdmissionError::InvalidSpecification)
    ));
    assert!(matches!(
        h.registry.begin_stop(key, ShutdownId::new(2)?, budget),
        Err(RegistryAdmissionError::InvalidSpecification)
    ));
    let mut foreign = key;
    foreign.owner.epoch = CoordinatorEpoch::new([2; 16])?;
    assert!(matches!(h.registry.snapshot(foreign), Err(RegistryAdmissionError::ForeignOwner)));
    let stale = ProcessKey { owner: key.owner, id: ProcessId::new(999)? };
    assert!(matches!(h.registry.snapshot(stale), Err(RegistryAdmissionError::Stale)));
    assert_eq!(h.registry.snapshot(key)?.shutdown, Some((ShutdownId::new(1)?, budget)));
    Ok(())
}

#[test]
fn registry_identity_exhaustion_and_fake_metadata_never_create_effects() -> TestResult {
    let mut h = Harness::new()?;
    h.registry.next_process = u64::MAX;
    assert!(h.launch(1).is_err());
    let batch = CompletionBatch {
        notifications: vec![sugarsubstitute_contracts::execution::CompletionNotification {
            scope_id: ScopeId::new(1)?,
            job_id: JobId::new(1)?,
            kind: sugarsubstitute_contracts::execution::TerminalKind::Success,
            replacement: None,
        }],
        remaining: 0,
    };
    assert_eq!(h.registry.consume(&batch)?, 0);
    assert!(h.registry.is_empty());
    assert_eq!(h.fake.creates.load(Ordering::Acquire), 0);
    Ok(())
}

#[test]
fn registry_error_redacts_outer_text_and_bounds_all_secondary_causes() -> TestResult {
    let mut error =
        RegistryError::new("create_process", std::io::Error::other("private E:\\secret token"));
    for _ in 0..16 {
        error.retain(RegistryError::invalid("close_job"));
    }
    assert_eq!(error.cleanup_causes().len(), 8);
    assert_eq!(error.omitted_causes(), 8);
    assert!(!format!("{error:?} {error}").contains("secret"));
    assert!(error.source().ok_or("Missing cause")?.to_string().contains("secret"));
    let error = RegistryAdmissionError::Execution(
        sugarsubstitute_contracts::execution::AdmissionError::Unexpected(ExecutionError::new(
            Operation::NativeLifecycle,
            Box::new(std::io::Error::other("private token")),
        )),
    );
    assert!(!format!("{error:?} {error}").contains("token"));
    assert!(error.source().is_some());
    Ok(())
}
