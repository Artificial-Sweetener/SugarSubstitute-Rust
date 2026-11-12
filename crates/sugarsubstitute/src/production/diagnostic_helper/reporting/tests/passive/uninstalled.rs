// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retire a real never-launched Parent while preserving an original uninstalled View and failure.
use super::*;
use crate::production::reporter_role::{parent::ReporterFunding, parent_source::ReporterSource};
use sugarsubstitute_execution::windows_purpose_pipe::MIN_LIFECYCLE_PIPE_BYTES;
use sugarsubstitute_supervision::WindowsProcessRegistry;

#[test]
fn reporting_uninstalled_view_survives_failure_and_real_never_launched_parent_retirement()
-> TestResult {
    let (owner, wake, scope) = execution()?;
    let client = owner.client();
    let mut controller = client.submit_blocking(&scope, spec(1, 2 * 1024 * 1024)?, |context| {
        let service = ReportingService::new(&context)?;
        let mut funding = Some(ReporterFunding::new(&context)?);
        Ok::<_, TestError>((service, ReporterSource::new(&mut funding)?))
    })?;
    let (service, source) = success(completed(&client, &wake, &mut controller)?)?;
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
    let namespace = entry.take_namespace().ok_or("Actual uninstalled View namespace missing")?;
    let binding = binding(&original()?)?;
    let coordinator = binding.incident().subject().coordinator();
    let operation = LogicalStoreOperationKey::new(coordinator, IncidentOperationId::new(1)?);
    let input = ReportingInput {
        request: namespace.diagnostic_write_request(operation)?,
        operation,
        work: ReportingWork::View { binding, locale: "en", root_capture_degraded: false },
        authentication: None,
    };
    let original_input = std::ptr::from_ref(&input);
    assert_eq!(service.passive_restoration(), None);
    assert!(!service.files_retired());
    assert_eq!(service.outcome(), None);
    assert_eq!(service.cleanup_outcome(), None);
    assert!(!service.cleanup_started());
    service.validate_retiring_input(operation, binding, false, &input)?;
    assert_eq!(
        service.validate_retiring_input(operation, binding, true, &input),
        Err(ReportingIssue::Busy)
    );
    let changed = LogicalStoreOperationKey::new(coordinator, IncidentOperationId::new(2)?);
    assert_eq!(
        service.validate_retiring_input(changed, binding, false, &input),
        Err(ReportingIssue::Identity)
    );
    let alias = service.source.alias();
    assert_eq!(
        service.validate_retiring_input(operation, binding, false, &input),
        Err(ReportingIssue::Busy)
    );
    drop(alias);
    let mut registry_job = WindowsProcessRegistry::submit_new(
        &client,
        &scope,
        spec(3, WindowsProcessRegistry::required_reservation_bytes())?,
        coordinator,
    )?;
    let registry = success(completed(&client, &wake, &mut registry_job)?)?;
    drop(registry_job);
    let native =
        client.create_native_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
    static NEXT_SESSION: AtomicU64 = AtomicU64::new(1);
    let sequence = NEXT_SESSION
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
            value.checked_add(1).filter(|next| *next <= u64::from(u32::MAX))
        })
        .map_err(|_| "Diagnostic test session identities exhausted")?;
    let key = DiagnosticKey::new(
        coordinator,
        DiagnosticRole::Reporter,
        DiagnosticSessionId::new((u64::from(std::process::id()) << 32) | sequence)?,
    );
    let mut pipe_spec = spec(4, MIN_LIFECYCLE_PIPE_BYTES)?;
    pipe_spec.class = JobClass::NativeObservation;
    pipe_spec.deadline = None;
    let mut session = Some(client.reserve_purpose_pipe_server(
        &native,
        pipe_spec,
        sugarsubstitute_execution::purpose_pipe::PurposeKey::Diagnostic(key),
        Moment::from_nanos(60_000_000_017),
    )?);
    source.install_owner(&mut session, key)?;
    assert!(session.is_none());
    prepared_parent(&client, &wake, &source)?;
    let mut failed = client.submit_blocking(&scope, spec(5, 16 * 1024)?, |_| {
        Err::<(), _>(std::io::Error::other("Original before-admission receiving failure"))
    })?;
    let failure = completed(&client, &wake, &mut failed)?;
    drop(failed);
    let JobOutcome::Failure(cause) = &failure.outcome else {
        return Err("Actual original receiving worker did not fail".into());
    };
    let original_cause = std::ptr::from_ref(cause);
    source.close()?;
    service.validate_retiring_input(operation, binding, false, &input)?;
    assert!(source.return_closed_inputs(&registry)?);
    retired_parent(&client, &wake, &source)?;
    assert!(source.poll_retiring()?.pipe_settled);
    assert_eq!(std::ptr::from_ref(&input), original_input);
    assert_eq!(input.operation, operation);
    assert_eq!(input.binding(), binding);
    assert!(input.authentication.is_none());
    assert_eq!(service.passive_restoration(), None);
    assert!(!service.files_retired());
    assert_eq!(service.cleanup_outcome(), None);
    entry.close_admission();
    entry.submit_close(&client, &scope, entry_spec(6, Moment::from_nanos(100))?)?;
    assert!(matches!(entry_delivery(&client, &wake, &mut entry)?, EntryObservation::Retired));
    assert!(namespace.validate_live().is_err());
    drop(namespace);
    drop(entry);
    drop(owner);
    assert_eq!(native.snapshot().reserved_bytes, MIN_LIFECYCLE_PIPE_BYTES);
    assert_eq!(
        scope.snapshot().reserved_bytes,
        2 * 1024 * 1024 + WindowsProcessRegistry::required_reservation_bytes() + 16 * 1024
    );
    let JobOutcome::Failure(cause) = &failure.outcome else {
        return Err("Original receiving failure was replaced during retirement".into());
    };
    assert_eq!(std::ptr::from_ref(cause), original_cause);
    assert_eq!(cause.to_string(), "Original before-admission receiving failure");
    drop(source);
    assert_eq!(native.snapshot().reserved_bytes, 0);
    drop(registry);
    drop(service);
    // Namespace close retired its native preparation credit; the unchanged inert request remains
    // owned here until disposal, independently of the retained original failure completion.
    assert_eq!(input.operation, operation);
    assert_eq!(input.binding(), binding);
    assert_eq!(scope.snapshot().reserved_bytes, 16 * 1024);
    drop(input);
    assert_eq!(scope.snapshot().reserved_bytes, 16 * 1024);
    drop(failure);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
