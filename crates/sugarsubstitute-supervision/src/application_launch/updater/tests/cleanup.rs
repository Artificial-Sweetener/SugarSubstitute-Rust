// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve completed channel retirement when process closure moves to independent cleanup.
use super::{fixture::*, *};
use crate::setup_storage::test_worker::pipeline::Pipeline;
use sugarsubstitute_contracts::execution::{
    ExecutionPort, JobClass, JobOutcome, JobSpec, Moment, Operation, ScopeId, ScopeSpec,
};

#[test]
fn retired_channel_survives_cancelled_startup_when_process_close_requires_cleanup() -> TestResult {
    close_retry(false)
}

#[test]
fn retired_channel_does_not_renew_cancelled_cleanup_with_a_still_owned_process_handle() -> TestResult
{
    close_retry(true)
}

/// Move a real failed native close across scopes; completed transport and unfinished cleanup
/// have different lifetimes even though both retain the same durable creation disposition.
fn close_retry(started_cleanup: bool) -> TestResult {
    let mut pipeline = Pipeline::new()?;
    let image = pipeline.run(
        storage::SIGNED_INSTALLER_RESERVATION_BYTES + storage::UPDATER_STAGING_RESERVATION_BYTES,
        move |context| {
            let image = Image::prepare(&context)?;
            image.launch.launch_with(&context, create, native::resume)?;
            let process = Process::observe(&image.launch)?;
            let protection = Protected::new(&image.launch)?;
            assert_eq!(process.wait()?, 0);
            let failure = image
                .launch
                .poll(&context, started_cleanup)
                .err()
                .ok_or("Protected close succeeded")?;
            assert_eq!(failure.operation(), "close_handle");
            protection.clear()?;
            assert!(std::fs::OpenOptions::new().write(true).open(&image.path).is_err());
            Ok(image)
        },
    )?;
    let cleanup = pipeline.admit(|client, original, _| {
        original.cancel();
        Ok(client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?)
    })?;
    let mut job = pipeline.admit(|client, _, id| {
        Ok(client.submit_blocking(
            &cleanup,
            JobSpec {
                id,
                operation: Operation::NativeLifecycle,
                class: JobClass::InfrastructureLifecycle,
                deadline: Some(Moment::from_nanos(2_000_000_000)),
                reservation_bytes: ACTIVATION_RESERVATION_BYTES,
                replacement: None,
            },
            move |context| -> Value<()> {
                if started_cleanup {
                    let failure = image
                        .launch
                        .poll(&context, true)
                        .err()
                        .ok_or("Cancelled cleanup renewed")?;
                    assert_eq!(failure.operation(), "activation_worker_admission");
                    assert!(std::fs::OpenOptions::new().write(true).open(&image.path).is_err());
                    return Ok(());
                }
                let expected = ActivationOutcome::Stopped {
                    exit_code: Some(0),
                    creation: CreationOutcome::NotCreated,
                };
                assert_eq!(image.launch.poll(&context, true)?, expected);
                assert_eq!(image.launch.poll(&context, true)?, expected);
                assert!(image.launch.launch(&context).is_err());
                assert!(std::fs::OpenOptions::new().write(true).open(&image.path).is_ok());
                Ok(())
            },
        )?)
    })?;
    match pipeline.observe(|| job.try_take_completion())?.outcome {
        JobOutcome::Success(()) => Ok(()),
        JobOutcome::Failure(cause) => Err(cause),
        other => Err(format!("Unexpected cleanup outcome: {:?}", other.kind()).into()),
    }
}
