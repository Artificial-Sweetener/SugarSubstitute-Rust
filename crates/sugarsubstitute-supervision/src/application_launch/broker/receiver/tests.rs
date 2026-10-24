// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Real file-object duplication, explicit inheritance, partial failure and retained creation facts.
use super::*;
use crate::setup_storage::test_worker::{self as worker, TestResult};
use std::{
    fs::{File, OpenOptions},
    os::windows::{
        fs::OpenOptionsExt,
        io::{AsRawHandle, FromRawHandle, OwnedHandle},
    },
};
use windows::{
    Win32::{
        Foundation::{
            GetHandleInformation, HANDLE_FLAG_INHERIT, HANDLE_FLAG_PROTECT_FROM_CLOSE,
            SetHandleInformation,
        },
        System::Threading::{CreateEventW, GetCurrentProcess},
    },
    core::PCWSTR,
};

/// Retain two different read-only disk objects; neither file is executed as an application.
fn inputs() -> Result<(tempfile::TempDir, File, File), Box<dyn std::error::Error + Send + Sync>> {
    let root = tempfile::tempdir()?;
    for name in ["selection", "payload"] {
        std::fs::write(root.path().join(name), name)?;
    }
    let read = |name| OpenOptions::new().read(true).share_mode(1).open(root.path().join(name));
    let selection = read("selection")?;
    let payload = read("payload")?;
    Ok((root, selection, payload))
}

/// Supply a controlled native source boundary; authentication is tested at the engine owner.
fn request(selection: &File, payload: &File, executable: &std::path::Path) -> wire::Request {
    wire::Request {
        version: 1,
        id: [7; 32],
        executable: executable.to_string_lossy().into_owned(),
        directory: executable
            .parent()
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_default(),
        entry: wire::Entry::Installed { selection: selection.as_raw_handle() as usize as u64 },
        pins: vec![payload.as_raw_handle() as usize as u64],
    }
}

/// Read an owned duplicate's actual native inheritance flag while its receiver remains locked.
fn inheritable(handle: &Handle) -> Result<bool, ClaimError> {
    let mut flags = 0;
    // SAFETY: the test retains the exact duplicate through its owner lock.
    unsafe { GetHandleInformation(handle.raw(), &mut flags) }
        .map_err(|cause| ClaimError::new("fixture_handle_flags", cause))?;
    Ok(flags & HANDLE_FLAG_INHERIT.0 != 0)
}

#[test]
fn native_capture_remaps_entry_and_keeps_payload_pinned_after_source_files_close() -> TestResult {
    worker::run_reserved(TRANSFER_RESERVATION_BYTES * 3, |context, _| {
        let (root, selection, payload) = inputs()?;
        let original = selection.as_raw_handle() as usize;
        let frame = request(&selection, &payload, &root.path().join("absent.exe")).encode()?;
        let receiver = LaunchReceiver::new([7; 32])?;
        // SAFETY: the borrowed current-process pseudo handle identifies this controlled source.
        receiver.capture_from(unsafe { GetCurrentProcess() }, &frame, context)?;
        {
            let state = crate::native_claims::lock(&receiver.0);
            let application =
                crate::native_claims::lock(&state.application.as_ref().ok_or("Capture absent")?.0);
            let Some(LaunchCustody::Broker(pins)) = &application.custody else {
                return Err("Pins absent".into());
            };
            assert_ne!(pins.inherited[0], original);
            assert!(inheritable(&pins.handles[0])?);
            assert!(!inheritable(&pins.handles[1])?);
            let command = String::from_utf16_lossy(
                &application.command.as_ref().ok_or("Command absent")?.command_line,
            );
            assert!(command.contains(&pins.inherited[0].to_string()));
        }
        drop(selection);
        drop(payload);
        assert!(OpenOptions::new().write(true).open(root.path().join("payload")).is_err());
        // SAFETY: the pseudo handle remains valid; replay must fail before any duplication.
        assert!(receiver.capture_from(unsafe { GetCurrentProcess() }, &frame, context).is_err());
        let result = receiver.finish(context)?;
        assert_eq!(result.outcome(), CreationOutcome::NotCreated);
        assert_eq!(wire::response(result.frame(), [7; 32])?, CreationOutcome::NotCreated);
        assert!(OpenOptions::new().write(true).open(root.path().join("payload")).is_ok());
        assert!(receiver.launch(context).is_err());
        assert_eq!(receiver.finish(context)?.outcome(), CreationOutcome::NotCreated);
        Ok(())
    })
}

#[test]
fn partial_duplication_failure_retains_every_output_until_checked_retirement() -> TestResult {
    worker::run_reserved(TRANSFER_RESERVATION_BYTES * 3, |context, _| {
        let (root, selection, payload) = inputs()?;
        // SAFETY: unnamed, non-inheritable event output is immediately adopted by this fixture.
        let raw = unsafe { CreateEventW(None, false, false, PCWSTR::null()) }?;
        // SAFETY: this uniquely owned event is not a disk object and must be rejected after adoption.
        let event = unsafe { OwnedHandle::from_raw_handle(raw.0) };
        let mut input = request(&selection, &payload, &root.path().join("absent.exe"));
        input.pins.push(event.as_raw_handle() as usize as u64);
        let receiver = LaunchReceiver::new([7; 32])?;
        // SAFETY: the controlled source owns each declared handle throughout duplication.
        let failure = receiver
            .capture_from(unsafe { GetCurrentProcess() }, &input.encode()?, context)
            .err()
            .ok_or("Event accepted")?;
        assert_eq!(failure.operation(), "launch_transfer_not_disk");
        assert_eq!(crate::native_claims::lock(&receiver.0).handles.len(), 3);
        drop(selection);
        drop(payload);
        drop(event);
        assert!(OpenOptions::new().write(true).open(root.path().join("payload")).is_err());
        assert!(receiver.launch(context).is_err());
        assert_eq!(receiver.finish(context)?.outcome(), CreationOutcome::NotCreated);
        assert!(OpenOptions::new().write(true).open(root.path().join("payload")).is_ok());
        Ok(())
    })
}

#[test]
fn rejected_creation_reports_not_created_only_after_retiring_native_pins() -> TestResult {
    worker::run_reserved(TRANSFER_RESERVATION_BYTES * 3, |context, _| {
        let (root, selection, payload) = inputs()?;
        let receiver = LaunchReceiver::new([7; 32])?;
        receiver.capture_from(
            // SAFETY: this worker retains the controlled source handles through native capture.
            unsafe { GetCurrentProcess() },
            &request(&selection, &payload, &root.path().join("absent.exe")).encode()?,
            context,
        )?;
        assert!(receiver.launch(context).is_err());
        assert!(receiver.launch(context).is_err());
        drop(selection);
        drop(payload);
        assert!(OpenOptions::new().write(true).open(root.path().join("payload")).is_err());
        assert_eq!(receiver.finish(context)?.outcome(), CreationOutcome::NotCreated);
        assert!(OpenOptions::new().write(true).open(root.path().join("payload")).is_ok());
        Ok(())
    })
}

#[test]
fn actual_creation_survives_creator_close_failure_without_a_negative_broker_reply() -> TestResult {
    worker::run_reserved(TRANSFER_RESERVATION_BYTES * 3, |context, _| {
        let (root, selection, payload) = inputs()?;
        let receiver = LaunchReceiver::new([7; 32])?;
        receiver.capture_from(
            // SAFETY: this worker retains the actual source file objects throughout duplication.
            unsafe { GetCurrentProcess() },
            &request(&selection, &payload, &std::env::current_exe()?).encode()?,
            context,
        )?;
        let application = crate::native_claims::lock(&receiver.0)
            .application
            .as_ref()
            .ok_or("Capture absent")?
            .clone();
        let failure = application
            .launch_with(context, |command, custody, output| {
                super::super::super::creation::create(command, custody, output)?;
                // SAFETY: the uniquely produced fixture process remains in its native output slot.
                unsafe {
                    SetHandleInformation(
                        output.hProcess,
                        HANDLE_FLAG_PROTECT_FROM_CLOSE.0,
                        HANDLE_FLAG_PROTECT_FROM_CLOSE,
                    )
                }
                .map_err(|cause| ClaimError::new("fixture_protect", cause))
            })
            .err()
            .ok_or("Protected process close succeeded")?;
        assert_eq!(failure.operation(), "close_handle");
        assert!(receiver.finish(context).is_err());
        {
            let state = crate::native_claims::lock(&application.0);
            let process = state
                .handles
                .iter()
                .find(|handle| handle.is_armed())
                .ok_or("Retained process absent")?;
            super::super::super::tests::settle_fixture(process.raw())?;
        }
        drop(selection);
        drop(payload);
        assert_eq!(receiver.finish(context)?.outcome(), CreationOutcome::Created);
        assert_eq!(receiver.finish(context)?.outcome(), CreationOutcome::Created);
        assert!(OpenOptions::new().write(true).open(root.path().join("payload")).is_ok());
        Ok(())
    })
}

#[test]
fn fresh_cleanup_scope_cannot_revive_cancelled_capture_but_can_retire_it() -> TestResult {
    use sugarsubstitute_contracts::execution::{
        ExecutionPort, JobClass, JobOutcome, JobSpec, Moment, Operation, ScopeId, ScopeSpec,
    };
    let mut pipeline = worker::pipeline::Pipeline::new()?;
    let (receiver, root) = pipeline.run(TRANSFER_RESERVATION_BYTES * 2, |context| {
        let (root, selection, payload) = inputs()?;
        let receiver = LaunchReceiver::new([7; 32])?;
        // SAFETY: controlled source files remain owned until all duplicates have been captured.
        let process = unsafe { GetCurrentProcess() };
        receiver.capture_from(
            process,
            &request(&selection, &payload, &root.path().join("absent.exe")).encode()?,
            &context,
        )?;
        Ok((receiver, root))
    })?;
    let copy = receiver.clone();
    let path = root.path().join("payload");
    let (mut job, _cleanup) = pipeline.admit(|client, original, id| {
        original.cancel();
        let cleanup =
            client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
        let job = client.submit_blocking(
            &cleanup,
            JobSpec {
                id,
                operation: Operation::NativeLifecycle,
                class: JobClass::InfrastructureLifecycle,
                deadline: Some(Moment::from_nanos(2_000_000_000)),
                reservation_bytes: TRANSFER_RESERVATION_BYTES,
                replacement: None,
            },
            move |context| -> TestResult {
                assert!(!context.cancellation_requested());
                assert_eq!(
                    copy.launch(&context).err().ok_or("Cancelled capture launched")?.operation(),
                    "launch_transfer_admission"
                );
                assert_eq!(copy.finish(&context)?.outcome(), CreationOutcome::NotCreated);
                assert!(OpenOptions::new().write(true).open(path).is_ok());
                Ok(())
            },
        )?;
        Ok((job, cleanup))
    })?;
    match pipeline.observe(|| job.try_take_completion())?.outcome {
        JobOutcome::Success(()) => Ok(()),
        JobOutcome::Failure(cause) => Err(cause),
        other => Err(format!("Unexpected cleanup outcome: {:?}", other.kind()).into()),
    }
}
