// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Restore a genuine protected View input and renew only its actual source primitive.
use super::*;
use crate::completion_wake::wait_while_watched;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use sugarsubstitute_contracts::{configuration::*, coordinator_entry::*};
use sugarsubstitute_supervision::coordinator_entry::{
    self, CoordinatorEntryOwner, EntryObservation,
};
fn entry_owner() -> Result<CoordinatorEntryOwner, TestError> {
    static NEXT_PATH: AtomicU64 = AtomicU64::new(1);
    let id = NEXT_PATH
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| value.checked_add(1))
        .map_err(|_| "Inert test path identity exhausted")?;
    let path: PathBuf =
        std::env::temp_dir().join(format!("entry-passive-inert-{}-{id}", std::process::id()));
    let configuration = StartupConfiguration::new(
        ConnectionConfiguration::Attach(LoopbackOrigin::parse("http://localhost:8188", 8188)?),
        ProfileId::new("Profile_1".to_owned())?,
        Presentation::new("en", Theme::Light, false)?,
    );
    let request = EntryLaunchRequest::new(
        configuration,
        path.clone(),
        path.clone(),
        path.clone(),
        path.clone(),
        path.clone(),
        vec![0, 0],
    )?;
    let input = coordinator_entry::CoordinatorEntryInput::new(
        path.clone(),
        path,
        sugarsubstitute_supervision::coordinator_entry::DiagnosticStorageLocation::Account,
        request,
    )?;
    Ok(coordinator_entry::namespace_fixture::namespace_owner(
        input,
        EntrySetupBudget::new(Moment::from_nanos(17))?,
    ))
}
fn entry_spec(id: u64, until: Moment) -> Result<JobSpec, TestError> {
    let mut spec = spec(id, coordinator_entry::ENTRY_PREPARATION_BYTES)?;
    spec.deadline = Some(until);
    Ok(spec)
}
#[test]
fn reporting_actual_protected_passive_view_retains_first_cause_and_same_claim_across_unique_generation()
-> TestResult {
    let (owner, wake, scope) = execution()?;
    let client = owner.client();
    let mut controller = client.submit_blocking(&scope, spec(1, 2 * 1024 * 1024)?, |context| {
        ReportingService::new(&context)
    })?;
    let mut service = success(completed(&client, &wake, &mut controller)?)?;
    drop(controller);
    let mut entry = entry_owner()?;
    let setup = entry.original_setup_budget();
    assert!(entry.take_namespace().is_none());
    entry.submit_discovery(&client, &scope, entry_spec(2, setup.until())?)?;
    assert!(matches!(
        entry_delivery(&client, &wake, &mut entry)?,
        EntryObservation::NamespacePrepared(_)
    ));
    let namespace = entry.take_namespace().ok_or("Actual consumed namespace missing")?;
    assert!(entry.take_namespace().is_none());
    let binding = binding(&original()?)?;
    assert_eq!(namespace.namespace(), binding.incident().subject().coordinator().namespace);
    let operation = LogicalStoreOperationKey::new(
        binding.incident().subject().coordinator(),
        IncidentOperationId::new(1)?,
    );
    let request = namespace.diagnostic_write_request(operation)?;
    let mut input = Some(ReportingInput {
        request,
        operation,
        work: ReportingWork::View { binding, locale: "en", root_capture_degraded: false },
        authentication: None,
    });
    let jobs = crate::job_sequence::JobSequence::new();
    jobs.allocate()?;
    jobs.allocate()?;
    let until = Moment::from_nanos(17);
    assert_eq!(
        service.submit(&client, &scope, &jobs, until, &mut input),
        Err(ReportingIssue::Admission)
    );
    assert!(input.is_none());
    assert!(
        matches!(service.admission_failure(), Some(AdmissionError::DeadlineExpired {deadline, now}) if *deadline == until && *now == until)
    );
    assert_eq!(service.outcome(), None);
    let marker = service.restore_unsubmitted(&mut input)?;
    assert_eq!(marker.original(), (scope.owner_id(), scope.id(), until));
    assert_eq!(marker.operation(), operation);
    assert_eq!(marker.binding(), binding);
    assert!(!marker.is_symbols());
    assert_eq!(marker.terminal(), None);
    assert_eq!(service.passive_restoration(), Some(marker));
    let restored = input.as_ref().ok_or("Actual restored View input missing")?;
    service.validate_retiring_input(operation, binding, false, restored)?;
    service.validate_retiring_input(operation, binding, true, restored)?;
    assert_eq!(input.as_ref().map(ReportingInput::binding), Some(binding));
    assert!(service.validate_passive_restoration(marker));
    let alias = service.source.alias();
    assert!(!service.validate_passive_restoration(marker));
    drop(alias);
    assert!(service.validate_passive_restoration(marker));
    assert_eq!(service.restore_unsubmitted(&mut input), Err(ReportingIssue::Busy));
    assert_eq!(service.source.install(&mut input), Err(ReportingIssue::Started));
    assert!(input.is_some());
    let mut adopted = None;
    assert!(matches!(service.renew_passive_adopted(&mut adopted), Err(ReportingIssue::Busy)));
    assert_eq!(service.passive_restoration(), Some(marker));
    assert!(adopted.is_none());
    service.source.replace_passive_generation()?;
    assert!(!service.validate_passive_restoration(marker));
    assert_eq!(service.source.replace_passive_generation(), Err(ReportingIssue::Busy));
    assert_eq!(service.passive_restoration(), Some(marker));
    assert!(service.admission_failure().is_some());
    assert_eq!(service.outcome(), None);
    assert_eq!(service.cleanup_outcome(), None);
    assert!(!service.files_retired());
    assert_eq!(scope.snapshot().reserved_bytes, 4 * 1024 * 1024);
    entry.close_admission();
    entry.submit_close(&client, &scope, entry_spec(4, Moment::from_nanos(100))?)?;
    assert!(matches!(entry_delivery(&client, &wake, &mut entry)?, EntryObservation::Retired));
    assert!(namespace.validate_live().is_err());
    drop(input);
    drop(namespace);
    drop(entry);
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, 2 * 1024 * 1024);
    drop(service);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[path = "passive/uninstalled.rs"]
mod uninstalled;

#[derive(Default)]
struct FilesystemGate {
    state: std::sync::Mutex<(usize, bool)>,
    changed: std::sync::Condvar,
}
impl FilesystemGate {
    fn read(&self) -> TestResult {
        let mut state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        state.0 += 1;
        self.changed.notify_all();
        drop(wait_while_watched(
            &self.changed,
            state,
            || "the test to release the external filesystem gate".to_owned(),
            |state| !state.1,
        ));
        Ok(())
    }
    fn both_reading(&self) -> TestResult {
        drop(wait_while_watched(
            &self.changed,
            self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner),
            || "both blocking workers to reach their external reads".to_owned(),
            |state| state.0 != 2,
        ));
        Ok(())
    }
    fn release(&self) {
        self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner).1 = true;
        self.changed.notify_all();
    }
}
/// Release the external reads even when the test fails before its explicit release.
struct ReleaseFilesystem(std::sync::Arc<FilesystemGate>);
impl Drop for ReleaseFilesystem {
    fn drop(&mut self) {
        self.0.release();
    }
}
#[test]
fn reporting_actual_queued_cancellation_restores_protected_input_and_keeps_original_completion_credit()
-> TestResult {
    let (owner, wake, scope) = execution()?;
    let client = owner.client();
    let mut controller = client.submit_blocking(&scope, spec(1, 2 * 1024 * 1024)?, |context| {
        ReportingService::new(&context)
    })?;
    let mut service = success(completed(&client, &wake, &mut controller)?)?;
    drop(controller);
    let mut entry = entry_owner()?;
    entry.submit_discovery(
        &client,
        &scope,
        entry_spec(2, entry.original_setup_budget().until())?,
    )?;
    assert!(matches!(
        entry_delivery(&client, &wake, &mut entry)?,
        EntryObservation::NamespacePrepared(_)
    ));
    let namespace = entry.take_namespace().ok_or("Actual cancellation namespace missing")?;
    let binding = binding(&original()?)?;
    let operation = LogicalStoreOperationKey::new(
        binding.incident().subject().coordinator(),
        IncidentOperationId::new(1)?,
    );
    let mut input = Some(ReportingInput {
        request: namespace.diagnostic_write_request(operation)?,
        operation,
        work: ReportingWork::View { binding, locale: "en", root_capture_degraded: false },
        authentication: None,
    });
    let gate = std::sync::Arc::new(FilesystemGate::default());
    let _release = ReleaseFilesystem(gate.clone());
    let mut reads = Vec::with_capacity(2);
    for id in [3, 4] {
        let boundary = gate.clone();
        let mut read_spec = spec(id, 1024)?;
        read_spec.operation = Operation::Infrastructure;
        read_spec.class = if id == 3 {
            JobClass::InfrastructureOrdinary
        } else {
            JobClass::InfrastructureLifecycle
        };
        reads.push(client.submit_blocking(&scope, read_spec, move |_| boundary.read())?);
    }
    // One ordinary and one lifecycle read occupy the actual two-worker pool.
    gate.both_reading()?;
    let jobs = crate::job_sequence::JobSequence::new();
    for _ in 0..4 {
        jobs.allocate()?;
    }
    let until = Moment::from_nanos(100);
    service.submit(&client, &scope, &jobs, until, &mut input)?;
    service.cancel();
    gate.release();
    let reads = reads.try_into().map_err(|_| "External read bound changed")?;
    let mut reads: [OwnedJob<(), TestError>; 2] = reads;
    let delivered = cancelled_delivery_with_reads(&client, &wake, &mut service, &mut reads)?;
    drop(delivered);
    drop(reads);
    drop(gate);
    assert_eq!(service.outcome(), Some(TerminalKind::Cancelled));
    assert_eq!(service.restore_unsubmitted(&mut input), Err(ReportingIssue::Busy));
    let marker = service.restore_unstarted(&mut input)?;
    assert_eq!(marker.terminal(), Some(TerminalKind::Cancelled));
    assert_eq!(marker.original(), (scope.owner_id(), scope.id(), until));
    assert_eq!(marker.operation(), operation);
    assert_eq!(marker.binding(), binding);
    assert!(!marker.is_symbols());
    assert_eq!(input.as_ref().map(ReportingInput::binding), Some(binding));
    assert_eq!(service.restore_unstarted(&mut input), Err(ReportingIssue::Busy));
    let mut adopted = None;
    assert!(matches!(service.renew_passive_adopted(&mut adopted), Err(ReportingIssue::Busy)));
    assert_eq!(service.outcome(), Some(TerminalKind::Cancelled));
    assert_eq!(service.cleanup_outcome(), None);
    assert!(!service.cleanup_started());
    assert!(!service.files_retired());
    // Read-worker completions can precede their final context disposal. Inspect
    // this retained delivery here; aggregate credit is checked after owner join.
    let completion = service.work.completion.as_ref().ok_or("Original cancelled delivery lost")?;
    assert_eq!(completion.memory.bytes(), DIAGNOSTIC_STORAGE_BYTES);
    assert_eq!(completion.spec.reservation_bytes, DIAGNOSTIC_STORAGE_BYTES);
    entry.close_admission();
    entry.submit_close(&client, &scope, entry_spec(6, until)?)?;
    assert!(matches!(entry_delivery(&client, &wake, &mut entry)?, EntryObservation::Retired));
    drop(input);
    drop(namespace);
    drop(entry);
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, 2 * 1024 * 1024 + DIAGNOSTIC_STORAGE_BYTES);
    drop(service);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
