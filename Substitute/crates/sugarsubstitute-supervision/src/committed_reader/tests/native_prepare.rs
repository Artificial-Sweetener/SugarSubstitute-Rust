// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify the real full-depth reader directory chain and original paired close custody.
use super::*;
use crate::{
    coordinator_entry::{
        CoordinatorEntryInput, DiagnosticStorageLocation,
        namespace_fixture::native_writer::{self, PinFault},
    },
    diagnostic_storage::{DIAGNOSTIC_STORAGE_BYTES, DiagnosticStorage},
};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use sugarsubstitute_contracts::configuration::{
    ConnectionConfiguration, LoopbackOrigin, Presentation, ProfileId, StartupConfiguration, Theme,
};
use sugarsubstitute_contracts::coordinator_entry::EntryLaunchRequest;
static NEXT: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Eq, PartialEq)]
enum Case {
    Complete,
    MissingChild,
    FailedReopenClose,
}

/// Fail only the external close primitive while the genuine counted OS handle remains live.
fn reject_verification_close(
    handle: &mut Handle,
    phase: &mut crate::durable_storage::protected::native_phase::NativePhase<'_>,
) -> Result<(), ClaimError> {
    let raw = handle.raw();
    let result = handle.close_in_phase_using(phase, |actual| {
        assert_eq!(actual, raw);
        Err(windows::core::Error::from_hresult(
            windows::Win32::Foundation::ERROR_ACCESS_DENIED.to_hresult(),
        ))
    });
    assert!(handle.is_armed());
    assert_eq!(handle.raw(), raw);
    result
}

/// Own a fresh real namespace, initialize its writer fence, then run one paired reader.
fn run(case: Case) -> TestResult {
    let mut rig = Harness::new()?;
    let temporary = std::env::temp_dir();
    let root = temporary.join(format!(
        "committed-reader-native-{}-{}",
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
    let input = CoordinatorEntryInput::new(
        path.clone(),
        path,
        DiagnosticStorageLocation::Explicit(root.join("explicit")),
        request,
    )?;
    rig.owner = native_writer::owner(input, rig.budget, root.clone(), PinFault::None);
    rig.discover()?;
    let namespace = rig.owner.take_namespace().ok_or("native namespace not consumed")?;
    let mut setup_spec = rig.spec(Moment::from_nanos(10_000_000_000))?;
    setup_spec.reservation_bytes = DIAGNOSTIC_STORAGE_BYTES;
    let mut setup = rig.client.submit_blocking(&rig.scope, setup_spec, move |context| {
        let mut storage = DiagnosticStorage::new(
            context
                .memory()
                .claim(DIAGNOSTIC_STORAGE_BYTES)
                .map_err(|cause| ClaimError::new("reader_native_setup_memory", cause))?,
        )
        .map_err(|cause| ClaimError::new("reader_native_setup_storage", cause))?;
        let key = operation(namespace.namespace())?;
        let operation = storage
            .begin_operation(key, &context)
            .map_err(|cause| ClaimError::new("reader_native_setup_operation", cause))?;
        let request = namespace
            .diagnostic_write_request(key)
            .map_err(|cause| ClaimError::new("reader_native_setup_request", cause))?;
        storage
            .resolve_namespace(&request, &operation, &context)
            .map_err(|cause| ClaimError::new("reader_native_setup_namespace", cause))?;
        storage
            .acquire_writer(&operation, &context)
            .map_err(|cause| ClaimError::new("reader_native_setup_writer", cause))?;
        storage
            .retire(&operation, &context)
            .map_err(|cause| ClaimError::new("reader_native_setup_retirement", cause))?;
        Ok::<_, ClaimError>(namespace)
    })?;
    let done = rig.consume_job(&mut setup)?;
    let namespace = match done.outcome {
        JobOutcome::Success(value) => value,
        other => return Err(format!("native setup failed: {:?}", other.kind()).into()),
    };
    drop(done.memory);
    drop(setup);
    let retained = Arc::new(Mutex::new(None::<CommittedDiagnosticReader>));
    let closing = retained.clone();
    let cleanup_client = rig.client.clone();
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
        .reserve_symbol_cleanup(
            &cleanup_scope,
            cleanup_spec,
            &main_scope,
            main_spec,
            move |context| {
                let reader = closing.lock().map_err(|_| missing())?.take().ok_or_else(missing)?;
                let proof = cleanup_client
                    .committed_reader_retirement(reader.original_origin(), &context)
                    .map_err(|cause| ClaimError::new("reader_native_cleanup_proof", cause))?;
                let original_close = if case == Case::FailedReopenClose {
                    let fields = reader.fields.as_ref().ok_or_else(missing)?;
                    assert_eq!(fields.directories.len(), 3);
                    assert!(fields.directories[2].is_armed());
                    Some(fields.failure.clone().ok_or_else(missing)?)
                } else {
                    None
                };
                match reader.retire(&proof) {
                    Ok(()) => Ok(original_close),
                    Err(failure) => {
                        *closing.lock().map_err(|_| missing())? = Some(failure.owner);
                        Err(failure.cause)
                    }
                }
            },
        )
        .map_err(|failure| failure.error)?;
    let worker = rig.client.clone();
    let main_retained = retained.clone();
    let mut main = rig
        .client
        .submit_symbol_helper(&main_scope, main_spec, &permit, move |context| {
            let mut reader = construct(&worker, &namespace, &context)?;
            let observation = context.native_handles().ok_or_else(missing)?.observation();
            assert_eq!(observation.snapshot(), (5, 5));
            let (fields, original) = reader.parts()?;
            if case == Case::MissingChild {
                let root = fields.namespace.resolved()?.diagnostic_directory().0;
                let namespace = fields.request.namespace();
                let hex =
                    crate::durable_storage::protected::diagnostic::namespace_component(namespace);
                let leaf = std::str::from_utf8(&hex).map_err(|_| missing())?;
                let directory = root.join("diagnostics").join("incident-v1").join(leaf);
                std::fs::remove_file(directory.join("writer.lock")).map_err(|cause| {
                    ClaimError::new("reader_native_remove_fixture_fence", cause)
                })?;
                std::fs::remove_dir(directory).map_err(|cause| {
                    ClaimError::new("reader_native_remove_fixture_child", cause)
                })?;
            }
            let close = if case == Case::FailedReopenClose {
                reject_verification_close
            } else {
                Handle::close_in_phase
            };
            let prepared = super::super::prepare::prepare(fields, original, close);
            if case == Case::MissingChild {
                assert!(prepared.is_err());
                assert!(!fields.ready);
                assert_eq!(fields.directories.len(), 4);
                assert!(fields.directories[..3].iter().all(Handle::is_armed));
                assert!(!fields.directories[3].is_armed());
                let first = fields.failure.as_ref().ok_or_else(missing)?;
                let original = std::error::Error::source(first).ok_or_else(missing)?;
                assert!(
                    original
                        .downcast_ref::<std::io::Error>()
                        .is_some_and(|cause| cause.raw_os_error() == Some(2))
                );
                assert_eq!(observation.snapshot(), (9, 10));
            } else if case == Case::FailedReopenClose {
                assert!(prepared.is_err());
                assert!(!fields.ready);
                assert_eq!(fields.directories.len(), 3);
                assert!(fields.directories.iter().all(Handle::is_armed));
                assert_eq!(observation.snapshot(), (8, 8));
                let cause = fields.failure.as_ref().ok_or_else(missing)?;
                assert!(std::error::Error::source(cause).is_some_and(|cause| {
                    cause.downcast_ref::<windows::core::Error>().is_some_and(|cause| {
                        cause.code() == windows::Win32::Foundation::ERROR_ACCESS_DENIED.to_hresult()
                    })
                }));
            } else {
                prepared?;
                assert!(fields.ready);
                assert_eq!(fields.directories.len(), 4);
                assert_eq!(fields.directories.capacity(), super::super::DIRECTORY_PINS);
                assert!(fields.directories.iter().all(Handle::is_armed));
                assert_eq!(observation.snapshot(), (9, 10));
                for index in 0..4 {
                    let mut phase = super::super::phase(
                        original,
                        &mut fields.scratch,
                        &mut fields.directory_causes[index],
                    );
                    let identity = crate::durable_storage::protected::paths::file_identity(
                        &fields.directories[index],
                        &mut phase,
                    )?;
                    assert!(identity.iter().any(|byte| *byte != 0));
                }
            }
            *main_retained.lock().map_err(|_| missing())? = Some(reader);
            Ok::<_, ClaimError>(observation)
        })
        .map_err(|failure| failure.error)?;
    let done = rig.consume_job(&mut main)?;
    let observation = match done.outcome {
        JobOutcome::Success(value) => value,
        other => return Err(format!("actual reader preparation failed: {:?}", other.kind()).into()),
    };
    let main_completion_native =
        done.native_handles.ok_or("Actual Main completion ledger absent")?;
    drop(done.memory);
    drop(main);
    let deadline = rig.client.freeze_symbol_cleanup_deadline(&permit, Duration::from_secs(15))?;
    let mut cleanup = permit.begin(deadline).map_err(|failure| failure.error)?;
    let done = rig.consume_job(&mut cleanup)?;
    let original_close = match done.outcome {
        JobOutcome::Success(value) => value,
        other => return Err(format!("actual reader cleanup failed: {:?}", other.kind()).into()),
    };
    drop(done.memory);
    drop(cleanup);
    assert_eq!(observation.snapshot().0, 0);
    drop(main_completion_native);
    if case == Case::FailedReopenClose {
        assert!(original_close.is_some());
        assert_eq!(main_scope.snapshot().reserved_bytes, main_spec.reservation_bytes);
    }
    drop(original_close);
    assert!(retained.lock().map_err(|_| "reader fixture mutex poisoned")?.is_none());
    rig.close(Moment::from_nanos(60_000_000_000))?;
    rig.join_execution_after_retirement()?;
    assert_eq!(main_scope.snapshot().reserved_bytes, 0);
    assert_eq!(cleanup_scope.snapshot().reserved_bytes, 0);
    assert_eq!(
        std::fs::canonicalize(&root)?.parent(),
        Some(std::fs::canonicalize(&temporary)?.as_path())
    );
    std::fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn full_depth_reader_retains_four_chain_pins_and_checks_each_transient_canonical_close()
-> TestResult {
    run(Case::Complete)
}
#[test]
fn partial_reader_open_retains_original_chain_and_returns_only_unused_reopen_credit() -> TestResult
{
    run(Case::MissingChild)
}
#[test]
fn canonical_reopen_failed_close_retains_exact_output_credit_and_original_cause() -> TestResult {
    run(Case::FailedReopenClose)
}
