// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise real pipe preparation, restricted child ends, drainage and EOF.
//! Native probes and writes run as admitted finite work. The production native
//! worker owns every pipe, pending read, callback and checked handle retirement.
use super::*;
use crate::{
    native::NativeObservationError,
    runtime_test_support::{Rig, TestResult, spec},
};
use std::{num::NonZeroUsize, os::windows::io::AsRawHandle};
use sugarsubstitute_contracts::{execution::*, lifecycle::*};
use windows::Win32::{Foundation::*, Storage::FileSystem::*};

const STDOUT: &[u8] = b"\r\x1b[31m\xf0\x9f\x8d\xac\x1b[0m\n";
const STDERR: &[u8] = b"\xce\xbb\r\nlast";

/// Keep matching facts independent of production-generated random endpoint names.
fn resource() -> TestResult<ResourceKey> {
    Ok(ResourceKey {
        owner: CoordinatorKey {
            namespace: NamespaceId::new([81; 32]),
            epoch: CoordinatorEpoch::new([82; 16])?,
        },
        id: ResourceId::new(1)?,
        generation: ResourceGeneration::new(1)?,
    })
}

/// Preserve the native cause at the exact failed qualification operation.
fn native_error(stage: &'static str, cause: windows::core::Error) -> NativeObservationError {
    NativeObservationError::new(stage, cause)
}

/// Require a successful byte-mode write, including the zero-byte probe.
fn write(handle: HANDLE, bytes: &[u8]) -> Result<(), NativeObservationError> {
    let mut written = 0;
    // SAFETY: the caller's stdio pin retains this synchronous endpoint; the
    // immutable bounded slice and writable count live through the native call.
    unsafe { WriteFile(handle, Some(bytes), Some(&mut written), None) }
        .map_err(|cause| native_error("output_abi_write", cause))?;
    if written as usize != bytes.len() {
        return Err(NativeObservationError::new(
            "output_abi_short_write",
            std::io::Error::other("Incomplete fixture write"),
        ));
    }
    Ok(())
}

/// Probe actual access masks without assuming source declarations establish rights.
fn check_child_ends(handles: [HANDLE; 3]) -> Result<(), NativeObservationError> {
    for handle in handles {
        let mut flags = 0;
        // SAFETY: the live stdio pin retains each handle; flags is writable.
        unsafe { GetHandleInformation(handle, &mut flags) }
            .map_err(|cause| native_error("output_abi_inherit", cause))?;
        if flags & HANDLE_FLAG_INHERIT.0 == 0 {
            return Err(NativeObservationError::new(
                "output_abi_inherit",
                std::io::Error::other("Child endpoint is not inheritable"),
            ));
        }
    }
    let mut input = [0; 1];
    let mut count = 1;
    // SAFETY: the NUL input is synchronous and pinned; the initialized byte and
    // count buffers are exactly the advertised writable ranges.
    unsafe { ReadFile(handles[0], Some(&mut input), Some(&mut count), None) }
        .map_err(|cause| native_error("output_abi_nul_read", cause))?;
    if count != 0 {
        return Err(NativeObservationError::new(
            "output_abi_nul_read",
            std::io::Error::other("NUL input did not report EOF"),
        ));
    }
    // SAFETY: the pinned NUL handle and one-byte immutable payload remain live.
    let denied = unsafe { WriteFile(handles[0], Some(b"x"), Some(&mut count), None) };
    require_denied(denied, "output_abi_nul_write")?;
    for handle in &handles[1..] {
        // SAFETY: each pinned synchronous write endpoint receives only a bounded
        // access probe; read rights must be rejected before any pipe read occurs.
        let denied = unsafe { ReadFile(*handle, Some(&mut input), Some(&mut count), None) };
        require_denied(denied, "output_abi_write_end_read")?;
    }
    Ok(())
}

/// Keep unexpected native denial causes rather than translating all failures to pass.
fn require_denied(
    result: windows::core::Result<()>,
    stage: &'static str,
) -> Result<(), NativeObservationError> {
    match result {
        Err(cause) if cause.code() == windows::core::HRESULT::from_win32(ERROR_ACCESS_DENIED.0) => {
            Ok(())
        }
        Err(cause) => Err(native_error(stage, cause)),
        Ok(()) => Err(NativeObservationError::new(
            stage,
            std::io::Error::other("Restricted access unexpectedly succeeded"),
        )),
    }
}

/// Real zero-length writes must not become EOF before subsequent bytes arrive.
#[test]
fn ffi_actual_stdio_rights_raw_drain_and_dual_eof_retire() -> TestResult {
    // Every completed nonempty read contains at least one byte, so even the
    // most fragmented scheduling cannot overflow the queue before consumption.
    assert!(STDOUT.len() + STDERR.len() <= MAX_WINDOWS_OUTPUT_CHUNKS);
    let rig = Rig::new()?;
    let scope =
        rig.client.create_native_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
    let resource = resource()?;
    let mut output_spec = spec(1, JobClass::NativeObservation, MIN_WINDOWS_OUTPUT_BYTES)?;
    output_spec.operation = Operation::NativeLifecycle;
    let mut session = rig.client.reserve_windows_output(&scope, output_spec, resource)?;
    loop {
        let observed = rig.wake.count();
        rig.client.drain_completions(NonZeroUsize::MIN)?;
        if session.control.snapshot().prepared {
            break;
        }
        if let Some(completion) = session.completion.try_take_completion() {
            return Err(
                format!("Actual output preparation failed: {:?}", completion.outcome).into()
            );
        }
        rig.wake.wait_after(observed)?;
    }
    let stdio = session.control.take_child_stdio()?;
    let mut writer = rig.client.submit_blocking(
        &rig.scope,
        spec(2, JobClass::InfrastructureOrdinary, 4096)?,
        move |_| {
            let inheritance = stdio
                .borrow_inheritance(resource)
                .map_err(|cause| NativeObservationError::new("output_abi_borrow", cause))?;
            let handles = [inheritance.stdin, inheritance.stdout, inheritance.stderr]
                .map(|handle| HANDLE(handle.as_raw_handle()));
            check_child_ends(handles)?;
            write(handles[1], b"")?;
            for byte in STDOUT {
                write(handles[1], std::slice::from_ref(byte))?;
            }
            for byte in STDERR {
                write(handles[2], std::slice::from_ref(byte))?;
            }
            // No child was created: returning drops the exact pin, after which the
            // native owner closes its writer copies and observes actual broken pipes.
            Ok::<(), NativeObservationError>(())
        },
    )?;
    let written = rig.completion(&mut writer)?;
    assert!(matches!(written.outcome, JobOutcome::Success(())), "{:?}", written.outcome);
    let completion = rig.completion(&mut session.completion)?;
    let JobOutcome::Success(report) = completion.outcome else {
        return Err(format!("Actual output ended unexpectedly: {:?}", completion.outcome).into());
    };
    assert!(report.retired);
    assert_eq!(report.resource, resource);
    assert_eq!(report.eof, [true; 2]);
    let mut captured = [Vec::new(), Vec::new()];
    let mut sequences = [0; 2];
    let mut ended = [false; 2];
    while let Some(loan) = session.control.take_batch()? {
        let batch = loan.batch().ok_or("Actual output batch storage missing")?;
        let stream = batch.stream().index();
        assert!(!ended[stream]);
        assert_eq!(batch.resource(), resource);
        assert_eq!(batch.sequence().get(), sequences[stream] + 1);
        assert!(batch.gap().is_none());
        sequences[stream] = batch.sequence().get();
        captured[stream].extend_from_slice(batch.bytes());
        ended[stream] = batch.end();
    }
    assert_eq!(ended, [true; 2]);
    assert_eq!(captured[0], STDOUT);
    assert_eq!(captured[1], STDERR);
    for (index, expected) in [STDOUT, STDERR].iter().enumerate() {
        assert_eq!(report.counters[index].read_bytes, expected.len() as u64);
        assert_eq!(report.counters[index].dropped_bytes, 0);
        assert_eq!(report.counters[index].dropped_batches, 0);
    }
    assert!(session.control.snapshot().retired);
    Ok(())
}
