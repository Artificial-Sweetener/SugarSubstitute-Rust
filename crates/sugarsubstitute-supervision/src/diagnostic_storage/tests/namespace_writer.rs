// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the genuine closed Diagnostic writer against actual retained Entry-native pins.
mod activation;
mod finished;
use super::*;
use crate::coordinator_entry::{
    self, CoordinatorEntryInput, DiagnosticStorageLocation, VerifiedEntryNamespace,
};
use coordinator_entry::namespace_fixture::native_writer::{self, PinFault};
use std::path::PathBuf;
use sugarsubstitute_contracts::configuration::{
    ConnectionConfiguration, LoopbackOrigin, Presentation, ProfileId, StartupConfiguration, Theme,
};
use sugarsubstitute_contracts::coordinator_entry::EntryLaunchRequest;
use sugarsubstitute_contracts::lifecycle::CoordinatorKey;
static NEXT: AtomicU64 = AtomicU64::new(1);
fn run(
    explicit: bool,
    fault: PinFault,
    extra: u64,
    work: impl FnOnce(VerifiedEntryNamespace, JobContext) -> TestResult + Send + 'static,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut rig = coordinator_entry::fixture::Harness::new()?;
    let root = std::env::temp_dir().join(format!(
        "diagnostic-entry-writer-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let configuration = StartupConfiguration::new(
        ConnectionConfiguration::Attach(LoopbackOrigin::parse("http://localhost:8188", 8188)?),
        ProfileId::new("Profile_1".to_owned())?,
        Presentation::new("en", Theme::Light, false)?,
    );
    let path = PathBuf::from(r"E:\inert-installed-image");
    let request = EntryLaunchRequest::new(
        configuration,
        path.clone(),
        path.clone(),
        path.clone(),
        path.clone(),
        path.clone(),
        vec![0, 0],
    )?;
    let location = if explicit {
        DiagnosticStorageLocation::Explicit(root.join("explicit"))
    } else {
        DiagnosticStorageLocation::Account
    };
    let input = CoordinatorEntryInput::new(path.clone(), path, location, request)?;
    rig.owner = native_writer::owner(input, rig.budget, root.clone(), fault);
    rig.discover()?;
    let namespace = rig.owner.take_namespace().ok_or("Actual Entry namespace not consumed")?;
    let mut spec = rig.spec(Moment::from_nanos(10_000_000_000))?;
    spec.reservation_bytes = DIAGNOSTIC_STORAGE_BYTES + extra;
    let mut job =
        rig.client.submit_blocking(&rig.scope, spec, move |context| work(namespace, context))?;
    let completion = rig.consume_job(&mut job)?;
    let outcome = completion.outcome;
    drop(completion.memory);
    drop(job);
    rig.close(Moment::from_nanos(60_000_000_000))?;
    rig.join_execution_after_retirement()?;
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    std::fs::remove_dir_all(root)?;
    match outcome {
        JobOutcome::Success(()) => Ok(()),
        JobOutcome::Failure(cause) => Err(cause),
        other => Err(format!("Actual writer outcome: {other:?}").into()),
    }
}
fn operation(
    namespace: &VerifiedEntryNamespace,
    id: u64,
) -> Result<LogicalStoreOperationKey, TestError> {
    Ok(LogicalStoreOperationKey::new(
        CoordinatorKey { namespace: namespace.namespace(), epoch: CoordinatorEpoch::new([1; 16])? },
        IncidentOperationId::new(id)?,
    ))
}
fn happy(explicit: bool) -> Result<(), Box<dyn std::error::Error>> {
    run(explicit, PinFault::None, 0, |namespace, context| {
        let mut storage =
            DiagnosticStorage::new(context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?)?;
        let key = operation(&namespace, 1)?;
        let operation = storage.begin_operation(key, &context)?;
        let request = namespace.diagnostic_write_request(key)?;
        let opening = storage.resolve_namespace(&request, &operation, &context)?;
        assert!(matches!(opening, DiagnosticStoreOpening::Created(_)));
        storage.acquire_writer(&operation, &context)?;
        storage.retire(&operation, &context)?;
        Ok(())
    })
}
#[test]
fn diagnostic_writer_account_selector_accepts_actual_entry_pin_under_closed_grant()
-> Result<(), Box<dyn std::error::Error>> {
    happy(false)
}
#[test]
fn diagnostic_writer_explicit_d_selector_accepts_actual_entry_pin_under_closed_grant()
-> Result<(), Box<dyn std::error::Error>> {
    happy(true)
}

fn native_grant(
    storage: &mut DiagnosticStorage,
    operation: &DiagnosticStorageOperation,
    context: &JobContext,
) -> Result<DiagnosticNativeGrant, DiagnosticStorageError> {
    let mut access = storage.check(operation, context)?;
    Ok(access.data_mut()?.allocation.native_grant())
}
#[test]
fn diagnostic_writer_rejects_closed_or_foreign_original_entry_pin_before_namespace_children()
-> Result<(), Box<dyn std::error::Error>> {
    for fault in [PinFault::Closed, PinFault::ForeignMemory] {
        run(true, fault, 0, |namespace, context| {
            let mut storage =
                DiagnosticStorage::new(context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?)?;
            let key = operation(&namespace, 1)?;
            let operation = storage.begin_operation(key, &context)?;
            let request = namespace.diagnostic_write_request(key)?;
            let rejected = storage
                .resolve_namespace(&request, &operation, &context)
                .err()
                .ok_or("Foreign or closed source pin admitted")?;
            assert_eq!(rejected.code(), Code::ForeignProtection);
            assert!(storage.snapshot().live_handles() > 0);
            storage.retire(&operation, &context)?;
            Ok(())
        })?;
    }
    Ok(())
}
#[test]
fn diagnostic_writer_loan_binds_exact_grant_operation_single_borrow_and_closed_source()
-> Result<(), Box<dyn std::error::Error>> {
    run(true, PinFault::None, DIAGNOSTIC_STORAGE_BYTES, |namespace, context| {
        use crate::coordinator_entry::read_namespace::NamespaceReadLoan;
        let mut storage =
            DiagnosticStorage::new(context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?)?;
        let key = operation(&namespace, 1)?;
        let operation = storage.begin_operation(key, &context)?;
        let original = native_grant(&mut storage, &operation, &context)?;
        let wrong = namespace.diagnostic_write_request(self::operation(&namespace, 2)?)?;
        assert!(NamespaceReadLoan::take_writer(&wrong, &original, &context).is_err());
        let request = namespace.diagnostic_write_request(key)?;
        let loan = NamespaceReadLoan::take_writer(&request, &original, &context)?;
        assert!(loan.validate_writer(&original, &context).is_ok());
        assert!(NamespaceReadLoan::take_writer(&request, &original, &context).is_err());
        let mut foreign =
            DiagnosticStorage::new(context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?)?;
        let foreign_operation = foreign.begin_operation(key, &context)?;
        let foreign_original = native_grant(&mut foreign, &foreign_operation, &context)?;
        assert!(loan.validate_writer(&foreign_original, &context).is_err());
        foreign.retire(&foreign_operation, &context)?;
        namespace.lifetime.close_admission();
        assert!(loan.validate_writer(&original, &context).is_err());
        drop(loan);
        storage.retire(&operation, &context)?;
        assert!(NamespaceReadLoan::take_writer(&request, &original, &context).is_err());
        Ok(())
    })
}
#[test]
fn genuine_committed_reader_loan_never_acquires_diagnostic_writer_authority()
-> Result<(), Box<dyn std::error::Error>> {
    use crate::coordinator_entry::read_namespace::NamespaceReadLoan;
    let mut rig = coordinator_entry::fixture::Harness::new()?;
    rig.discover()?;
    let namespace = rig.owner.take_namespace().ok_or("Actual namespace not consumed")?;
    let key = operation(&namespace, 1).map_err(|cause| -> Box<dyn std::error::Error> { cause })?;
    let mut spec = rig.spec(Moment::from_nanos(100))?;
    spec.reservation_bytes = DIAGNOSTIC_STORAGE_BYTES;
    let mut job = rig.client.submit_blocking(&rig.scope, spec, move |context| {
        let mut storage =
            DiagnosticStorage::new(context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?)?;
        let operation = storage.begin_operation(key, &context)?;
        let original = native_grant(&mut storage, &operation, &context)?;
        Ok::<_, TestError>((storage, operation, original))
    })?;
    let completion = rig.consume_job(&mut job)?;
    let (mut storage, operation, original) = match completion.outcome {
        JobOutcome::Success(value) => value,
        other => {
            return Err(
                format!("Original Diagnostic construction failed: {:?}", other.kind()).into()
            );
        }
    };
    drop(completion.memory);
    drop(job);
    let main_scope =
        rig.client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
    let cleanup_scope =
        rig.client.create_scope(ScopeSpec { id: ScopeId::new(3)?, deadline: None }, None)?;
    let mut cleanup_spec = rig.spec(Moment::from_nanos(100))?;
    cleanup_spec.operation = Operation::Infrastructure;
    cleanup_spec.deadline = None;
    cleanup_spec.reservation_bytes = 32_768;
    let mut main_spec = rig.spec(Moment::from_nanos(100))?;
    main_spec.operation = Operation::Infrastructure;
    main_spec.class = JobClass::InfrastructureOrdinary;
    main_spec.reservation_bytes = 8_159_232;
    let permit = rig
        .client
        .reserve_symbol_cleanup(&cleanup_scope, cleanup_spec, &main_scope, main_spec, |_| {
            Ok::<_, TestError>(())
        })
        .map_err(|reject| reject.error)?;
    let client = rig.client.clone();
    let mut main = rig
        .client
        .submit_symbol_helper(&main_scope, main_spec, &permit, move |context| {
            let request = namespace.committed_read_request(key)?;
            let reader = client.committed_reader_admission(&context)?.into_origin();
            let loan = NamespaceReadLoan::take(&request, &reader)?;
            assert!(loan.validate_writer(&original, &context).is_err());
            drop(loan);
            drop(reader);
            storage.retire(&operation, &context)?;
            Ok::<_, TestError>(())
        })
        .map_err(|reject| reject.error)?;
    let outcome = rig.consume_job(&mut main)?.outcome;
    drop(main);
    let mut cleanup = permit.cancel_and_observe();
    assert!(matches!(rig.consume_job(&mut cleanup)?.outcome, JobOutcome::Cancelled));
    drop(cleanup);
    rig.close(Moment::from_nanos(60_000_000_000))?;
    rig.join_execution_after_retirement()?;
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    match outcome {
        JobOutcome::Success(()) => Ok(()),
        other => Err(format!("Actual reader-only rejection failed: {other:?}").into()),
    }
}
