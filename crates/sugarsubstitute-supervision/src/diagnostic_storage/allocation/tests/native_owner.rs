// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject disguised native origins and preserve actual admitted control/cause custody.
use super::*;
use crate::{
    claim_error::BoundedNativeCause,
    durable_storage::protected::{
        native_file::{FileGrant, NativeCauseDestinations, NativeFile, NativeFileAdmission},
        native_name::NativeName,
        native_phase::NativePhase,
    },
    native_claims::files::Budget,
};
use std::path::PathBuf;

fn bound_empty(
    context: &JobContext,
) -> Result<(DiagnosticStorage, DiagnosticStorageOperation), TestError> {
    let memory = context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?;
    let mut storage = DiagnosticStorage::from_port(
        allocation::owner(memory)?,
        OwnedPort::new(Box::new(Filesystem::new()), None),
    )?;
    let operation = storage.begin_operation(key(1)?, context)?;
    Ok((storage, operation))
}

#[test]
fn finished_native_and_rust_roles_use_original_residual_and_keep_last_credit() -> TestResult {
    let cpp_bytes =
        size_of::<super::super::super::finished_owner::ProtectedFinishedOwnerAllocation>();
    let rust_bytes =
        size_of::<super::super::super::finished_owner::ProtectedFinishedRustRetention>();
    println!("finished_cpp_guard_bytes={cpp_bytes} finished_rust_guard_bytes={rust_bytes}");
    assert!(
        cpp_bytes + rust_bytes + 5056
            <= super::super::super::finished_owner::FINISHED_RUST_STORAGE_BYTES
    );
    admitted(|context, _| {
        let (mut storage, operation) = bound_empty(&context)?;
        let original = operation.shared._allocation.native_grant();
        let finished = original.finished_owner(&context)?;
        let rust = original.finished_rust_owner(&context)?;
        let rejected = original
            .finished_owner(&context)
            .err()
            .ok_or("Second four-MiB destination exceeded original residual")?;
        assert_eq!(rejected.code(), Code::Capacity);
        assert_eq!(rejected.operation(), Some(operation.key()));
        drop(finished);
        let successor = original.finished_owner(&context)?;
        storage.retire(&operation, &context)?;
        drop(operation);
        drop(storage);
        drop(original);
        assert!(context.memory().claim(1).is_err());
        drop(successor);
        assert!(context.memory().claim(1).is_err());
        drop(rust);
        assert_eq!(
            context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?.bytes(),
            DIAGNOSTIC_STORAGE_BYTES
        );
        Ok(())
    })
}

#[test]
fn diagnostic_native_ordinary_branches_reject_before_allocation_or_effect() -> TestResult {
    admitted(|context, _| {
        let (mut storage, operation) = bound_empty(&context)?;
        let original = operation.shared._allocation.native_grant();
        let budget = Budget::Diagnostic(original.clone());
        let rejection = NativePhase::ordinary(&budget)
            .err()
            .ok_or("Diagnostic origin entered ordinary phase")?;
        let source = std::error::Error::source(&rejection)
            .and_then(|cause| cause.downcast_ref::<DiagnosticNativeAllocationFailure>())
            .ok_or("Ordinary rejection allocated an ordinary cause")?;
        assert_eq!(source.operation(), Some(operation.key()));
        // The closed enum branch must reject even a direct internal representation.
        let mut disguised = NativePhase::Ordinary { budget: &budget };
        let failure = disguised
            .ntstatus("fixture_disguised_cng", windows::Win32::Foundation::NTSTATUS(-1))
            .err()
            .ok_or("Disguised ordinary CNG conversion accepted")?;
        assert!(
            std::error::Error::source(&failure)
                .is_some_and(|cause| cause.is::<DiagnosticNativeAllocationFailure>())
        );
        let failure = disguised.error(
            "fixture_disguised_leaf",
            BoundedNativeCause::RawIo(std::io::Error::from(std::io::ErrorKind::PermissionDenied)),
        );
        assert!(
            std::error::Error::source(&failure)
                .is_some_and(|cause| cause.is::<DiagnosticNativeAllocationFailure>())
        );

        drop(failure);
        let mut input = Some((
            NativeName::Ordinary(PathBuf::from("no-native-open")),
            NativeFileAdmission::Ordinary { budget },
        ));
        let failure = NativeFile::reserve(1, 0, FileGrant::MetadataReadOnly, &mut input)
            .err()
            .ok_or("Diagnostic origin entered ordinary native file")?;
        let (name, admission) = input.as_ref().ok_or("Rejected input was consumed")?;
        assert!(matches!(admission, NativeFileAdmission::Ordinary { .. }));
        assert_eq!(name.path()?, Path::new("no-native-open"));
        assert!(
            std::error::Error::source(&failure)
                .is_some_and(|cause| cause.is::<DiagnosticNativeAllocationFailure>())
        );
        drop(input);
        assert_eq!(storage.snapshot().live_handles(), 0);
        drop(failure);
        drop(rejection);
        drop(original);
        storage.retire(&operation, &context)?;
        Ok(())
    })
}

#[test]
fn diagnostic_native_scratch_rejects_wrong_nested_role_without_effect() -> TestResult {
    admitted(|context, _| {
        let (mut storage, operation) = bound_empty(&context)?;
        let original = operation.shared._allocation.native_grant();
        let mut account = original.account_scratch(&context)?;
        account.check_account(&context)?;
        assert_eq!(
            account.borrow_path().err().ok_or("Account admitted path")?.code(),
            Code::Admission
        );
        assert_eq!(
            account.borrow_security().err().ok_or("Account admitted security")?.code(),
            Code::Admission
        );
        let mut path = original.path_scratch(&context)?;
        assert_eq!(
            path.check_account(&context).err().ok_or("Path admitted token query")?.code(),
            Code::Admission
        );
        path.borrow_path()?.check(&context)?;
        path.borrow_security()?.check(&context)?;
        drop(path);
        drop(account);
        drop(original);
        storage.retire(&operation, &context)?;
        Ok(())
    })
}

#[test]
fn diagnostic_native_first_failure_observes_actual_expiry_and_retains_escaped_pair() -> TestResult {
    admitted_timeout(|context, clock| {
        let (storage, operation) = bound_empty(&context)?;
        let original = operation.shared._allocation.native_grant();
        let mut scratch = original.path_scratch(&context)?;
        let mut causes = NativeCauseDestinations::diagnostic(&original)?;
        let mut phase = NativePhase::Diagnostic {
            original: &original,
            current: &context,
            scratch: &mut scratch,
            causes: &mut causes,
            source: None,
        };
        let first = phase.error(
            "fixture_native_first",
            BoundedNativeCause::RawIo(std::io::Error::from(std::io::ErrorKind::PermissionDenied)),
        );
        // Expire the actual original provider after the finite native failure.
        clock.0.store(100, Ordering::SeqCst);
        let failure = phase
            .post_context::<()>(Err(first), &context)
            .err()
            .ok_or("Failed native result bypassed original control")?;
        let source = std::error::Error::source(&failure)
            .and_then(|cause| cause.downcast_ref::<std::io::Error>())
            .ok_or("Native first cause replaced")?;
        assert_eq!(source.kind(), std::io::ErrorKind::PermissionDenied);
        let secondary = failure.cleanup_cause().ok_or("Actual post-control failure missing")?;
        let source = std::error::Error::source(secondary)
            .and_then(|cause| cause.downcast_ref::<DiagnosticNativeAllocationFailure>())
            .ok_or("Actual original control cause replaced")?;
        assert_eq!(source.code(), Code::Deadline);

        assert!(causes.check(false).is_err());
        assert!(causes.check(true).is_err());
        assert_eq!(storage.snapshot().live_handles(), 0);
        drop(scratch);
        drop(causes);
        drop(original);
        drop(operation);
        drop(storage);
        assert!(context.memory().claim(1).is_err());
        let alias = failure.clone();
        drop(failure);
        assert!(context.memory().claim(1).is_err());
        drop(alias);
        assert_eq!(
            context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?.bytes(),
            DIAGNOSTIC_STORAGE_BYTES
        );
        Ok(())
    })
}

#[test]
fn diagnostic_native_named_peaks_cover_real_phase_buffers_within_original_residual() -> TestResult {
    use crate::durable_storage::protected::{account, paths, security};
    let path = paths::diagnostic_path_layout()?;
    let account = account::diagnostic_account_layout()?;
    let security = security::diagnostic_security_layout()?;
    let hash = NativeFile::diagnostic_hash_layout()?;
    assert_eq!(path.retained_bytes(), 0);
    assert_eq!(hash.retained_bytes(), 0);
    assert!(path.peak_bytes() > 262_130);
    assert!(account.peak_bytes() > 256 * size_of::<usize>());
    assert!(security.peak_bytes() > 9596);
    assert!(hash.peak_bytes() > 65_536);
    let simultaneous =
        path.peak_bytes() + account.peak_bytes() + security.peak_bytes() + hash.peak_bytes();
    assert!(simultaneous < DIAGNOSTIC_RESIDUAL_BYTES);
    assert_eq!(
        ClaimError::diagnostic_secondary_cause_layout()?.retained_bytes(),
        ClaimError::diagnostic_native_cause_layout()?.retained_bytes()
    );
    Ok(())
}

#[test]
fn diagnostic_native_cng_failure_keeps_code_only_status_and_held_cause() -> TestResult {
    use windows::Win32::Foundation::NTSTATUS;
    admitted(|context, _| {
        let (mut storage, operation) = bound_empty(&context)?;
        let original = operation.shared._allocation.native_grant();
        let mut scratch = original.path_scratch(&context)?;
        let mut causes = NativeCauseDestinations::diagnostic(&original)?;
        let mut phase = NativePhase::Diagnostic {
            original: &original,
            current: &context,
            scratch: &mut scratch,
            causes: &mut causes,
            source: None,
        };
        phase.ntstatus("fixture_cng_success", NTSTATUS(0))?;
        let status = NTSTATUS(-1_073_741_811);
        let failure = phase
            .ntstatus("fixture_cng_status", status)
            .err()
            .ok_or("Negative CNG status accepted")?;
        assert_eq!(failure.operation(), "fixture_cng_status");
        let source = std::error::Error::source(&failure)
            .and_then(|cause| cause.downcast_ref::<windows::core::Error>())
            .ok_or("CNG native status cause replaced")?;
        assert_eq!(source.code(), status.to_hresult());
        assert!(source.as_ptr().is_null());

        assert!(causes.check(false).is_err());
        causes.check(true)?;
        assert_eq!(storage.snapshot().live_handles(), 0);
        drop(failure);
        drop(causes);
        drop(scratch);
        drop(original);
        storage.retire(&operation, &context)?;
        Ok(())
    })
}

#[test]
fn funded_native_primary_preserves_inline_secondary_and_last_alias_credit() -> TestResult {
    admitted(|context, _| {
        let (mut storage, operation) = bound_empty(&context)?;
        let original = operation.shared._allocation.native_grant();
        let destination = crate::claim_error::NativeCauseDestination::diagnostic(
            original.native_cause()?,
            original.clone(),
        );
        let primary = ClaimError::native(
            "mixed_primary",
            BoundedNativeCause::RawIo(std::io::Error::from(std::io::ErrorKind::PermissionDenied)),
            destination,
        );
        let secondary = ClaimError::diagnostic_allocation(
            DiagnosticNativeAllocationFailure::rejected(Some(key(2)?), Code::Capacity),
        );
        let cause = ClaimError::finish::<()>(Err(primary), Err(secondary))
            .err()
            .ok_or("Mixed rejection succeeded")?;
        assert_eq!(
            std::error::Error::source(&cause)
                .and_then(|source| source.downcast_ref::<std::io::Error>())
                .ok_or("Native primary replaced")?
                .kind(),
            std::io::ErrorKind::PermissionDenied
        );
        let secondary = cause
            .cleanup_cause()
            .and_then(std::error::Error::source)
            .and_then(|source| source.downcast_ref::<DiagnosticNativeAllocationFailure>())
            .ok_or("Inline secondary replaced")?;
        assert_eq!(secondary.operation(), Some(key(2)?));
        assert_eq!(secondary.code(), Code::Capacity);
        let alias = cause.clone();
        storage.retire(&operation, &context)?;
        drop(operation);
        drop(storage);
        drop(original);
        drop(cause);
        assert!(context.memory().claim(1).is_err());
        drop(alias);
        assert_eq!(
            context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?.bytes(),
            DIAGNOSTIC_STORAGE_BYTES
        );
        Ok(())
    })
}

#[test]
fn funded_native_secondary_preserves_inline_primary_and_last_alias_credit() -> TestResult {
    admitted(|context, _| {
        let (mut storage, operation) = bound_empty(&context)?;
        let original = operation.shared._allocation.native_grant();
        let primary = ClaimError::diagnostic_allocation(
            DiagnosticNativeAllocationFailure::rejected(Some(key(2)?), Code::Capacity),
        );
        let destination = crate::claim_error::NativeCauseDestination::diagnostic(
            original.native_secondary_cause()?,
            original.clone(),
        );
        let secondary = ClaimError::native(
            "mixed_secondary",
            BoundedNativeCause::RawIo(std::io::Error::from(std::io::ErrorKind::PermissionDenied)),
            destination,
        );
        let cause = ClaimError::finish::<()>(Err(primary), Err(secondary))
            .err()
            .ok_or("Mixed rejection succeeded")?;
        let primary = std::error::Error::source(&cause)
            .and_then(|source| source.downcast_ref::<DiagnosticNativeAllocationFailure>())
            .ok_or("Inline primary replaced")?;
        assert_eq!(primary.operation(), Some(key(2)?));
        assert_eq!(primary.code(), Code::Capacity);
        assert_eq!(
            cause
                .cleanup_cause()
                .and_then(std::error::Error::source)
                .and_then(|source| source.downcast_ref::<std::io::Error>())
                .ok_or("Native secondary replaced")?
                .kind(),
            std::io::ErrorKind::PermissionDenied
        );
        let alias = cause.clone();
        storage.retire(&operation, &context)?;
        drop(operation);
        drop(storage);
        drop(original);
        drop(cause);
        assert!(context.memory().claim(1).is_err());
        drop(alias);
        assert_eq!(
            context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?.bytes(),
            DIAGNOSTIC_STORAGE_BYTES
        );
        Ok(())
    })
}

#[test]
fn rejected_native_name_and_sid_slots_keep_exact_original_inputs() -> TestResult {
    admitted(|context, _| {
        use crate::durable_storage::protected::native_name::NativeSid;
        let (mut storage, operation) = bound_empty(&context)?;
        let original = operation.shared._allocation.native_grant();
        let mut path = PathBuf::from(r"E:\fixture-bounded-name");
        path.try_reserve_exact(192 * 1024 + 1)?;
        let path_capacity = path.capacity();
        let mut name_input = Some((
            path,
            original.owned_name(&context)?,
            original.namespace_control()?,
            original.clone(),
        ));
        let before = storage.shared._allocation.fixture_residual_bytes();
        let rejection =
            NativeName::retained(&mut name_input).err().ok_or("Oversized name accepted")?;
        assert_eq!(storage.shared._allocation.fixture_residual_bytes(), before);
        let (retained, _, _, retained_origin) =
            name_input.as_ref().ok_or("Rejected name consumed")?;
        assert_eq!(retained.capacity(), path_capacity);
        assert_eq!(retained.as_os_str(), std::ffi::OsStr::new(r"E:\fixture-bounded-name"));
        assert!(retained_origin.matches_original(&original));
        assert!(
            std::error::Error::source(&rejection)
                .is_some_and(|cause| cause.is::<DiagnosticNativeAllocationFailure>())
        );
        drop(rejection);
        drop(name_input);
        let mut bytes = Vec::with_capacity(73);
        bytes.extend_from_slice(&[1, 2, 3]);
        let mut sid_input = Some((bytes, original.account_sid(&context)?, original.clone()));
        let rejection =
            NativeSid::retained(&mut sid_input).err().ok_or("Oversized SID accepted")?;
        let (bytes, _, retained_origin) = sid_input.as_ref().ok_or("Rejected SID consumed")?;
        assert_eq!(bytes.as_slice(), &[1, 2, 3]);
        assert!(retained_origin.matches_original(&original));
        drop(rejection);
        drop(sid_input);
        let mut input = Some((
            PathBuf::from(r"E:\fixture-bounded-name"),
            original.owned_name(&context)?,
            original.namespace_control()?,
            original.clone(),
        ));
        let retained = NativeName::retained(&mut input)?;
        assert!(input.is_none());
        assert_eq!(retained.path()?, Path::new(r"E:\fixture-bounded-name"));
        drop(retained);
        drop(original);
        storage.retire(&operation, &context)?;
        Ok(())
    })
}
