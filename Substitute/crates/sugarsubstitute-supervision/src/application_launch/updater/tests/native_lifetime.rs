// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Actual Windows creation, containment, failure and durable transfer qualification.
use super::{fixture::*, *};

#[test]
fn successful_engine_exit_without_a_request_proves_no_application_creation() -> TestResult {
    with_image(|launch, path, context| {
        launch.launch_with(context, create, native::resume)?;
        let process = Process::observe(&launch)?;
        assert_eq!(process.wait()?, 0);
        assert_eq!(
            launch.poll(context, false)?,
            ActivationOutcome::Stopped {
                exit_code: Some(0),
                creation: CreationOutcome::NotCreated
            }
        );
        assert_eq!(
            launch.poll(context, true)?,
            ActivationOutcome::Stopped {
                exit_code: Some(0),
                creation: CreationOutcome::NotCreated
            }
        );
        assert!(launch.launch(context).is_err());
        assert!(std::fs::OpenOptions::new().write(true).open(path).is_ok());
        Ok(())
    })
}

#[test]
fn partial_creation_outputs_remain_contained_until_all_members_settle() -> TestResult {
    with_image(|launch, path, context| {
        let failure = launch
            .launch_with(
                context,
                |command, job, channel, output| {
                    create(command, job, channel, output)?;
                    Err(ClaimError::invalid("fixture_partial_creation"))
                },
                native::resume,
            )
            .err()
            .ok_or("Partial creation accepted")?;
        assert_eq!(failure.operation(), "fixture_partial_creation");
        let process = Process::observe(&launch)?;
        let member = Process::member(&launch)?;
        assert!(process.running()? && member.running()?);
        assert_eq!(launch.poll(context, false)?, ActivationOutcome::Pending);
        assert!(std::fs::OpenOptions::new().write(true).open(path).is_err());
        let first = launch.poll(context, true)?;
        assert!(matches!(
            first,
            ActivationOutcome::Settling { .. } | ActivationOutcome::Stopped { .. }
        ));
        assert_eq!(process.wait()?, 1);
        assert_eq!(member.wait()?, 1);
        assert!(matches!(launch.poll(context, true)?, ActivationOutcome::Stopped { .. }));
        assert!(std::fs::OpenOptions::new().write(true).open(path).is_ok());
        Ok(())
    })
}

#[test]
fn failed_resume_cannot_transfer_and_owner_loss_kills_its_actual_child() -> TestResult {
    with_image(|launch, _, context| {
        let failure = launch
            .launch_with(context, create, |_| Err(ClaimError::invalid("fixture_resume")))
            .err()
            .ok_or("Failed resume accepted")?;
        assert_eq!(failure.operation(), "fixture_resume");
        let process = Process::observe(&launch)?;
        assert!(process.running()?);
        drop(launch);
        process.wait()?;
        Ok(())
    })
}

#[test]
fn actual_nonzero_exit_settles_descendants_before_fallback() -> TestResult {
    with_image(|launch, _, context| {
        launch.launch_with(context, failing, native::resume)?;
        let process = Process::observe(&launch)?;
        let member = Process::member(&launch)?;
        let code = process.wait()?;
        assert_ne!(code, 0);
        let first = launch.poll(context, false)?;
        assert!(matches!(
            first,
                ActivationOutcome::Settling { exit_code: Some(actual) }
                    | ActivationOutcome::Stopped { exit_code: Some(actual), .. } if actual == code
        ));
        assert_eq!(member.wait()?, 1);
        assert_eq!(
            launch.poll(context, true)?,
            ActivationOutcome::Stopped {
                exit_code: Some(code),
                creation: CreationOutcome::NotCreated
            }
        );
        Ok(())
    })
}

#[test]
fn successful_engine_exit_still_settles_all_engine_descendants() -> TestResult {
    with_image(|launch, _, context| {
        launch.launch_with(context, create, native::resume)?;
        let process = Process::observe(&launch)?;
        let member = Process::member(&launch)?;
        assert_eq!(process.wait()?, 0);
        let first = launch.poll(context, false)?;
        assert!(matches!(
            first,
            ActivationOutcome::Settling { .. } | ActivationOutcome::Stopped { .. }
        ));
        assert_eq!(member.wait()?, 1);
        assert_eq!(
            launch.poll(context, true)?,
            ActivationOutcome::Stopped {
                exit_code: Some(0),
                creation: CreationOutcome::NotCreated
            }
        );
        drop(launch);
        assert!(!member.running()?);
        Ok(())
    })
}

#[test]
fn failed_native_close_retains_settlement_and_authenticated_pins_until_retry() -> TestResult {
    with_image(|launch, path, context| {
        launch.launch_with(context, create, native::resume)?;
        let process = Process::observe(&launch)?;
        let protection = Protected::new(&launch)?;
        assert_eq!(process.wait()?, 0);
        let failure = launch.poll(context, false).err().ok_or("Protected closure succeeded")?;
        assert_eq!(failure.operation(), "close_handle");
        assert!(std::fs::OpenOptions::new().write(true).open(path).is_err());
        protection.clear()?;
        let expected = ActivationOutcome::Stopped {
            exit_code: Some(0),
            creation: CreationOutcome::NotCreated,
        };
        assert_eq!(launch.poll(context, true)?, expected);
        assert_eq!(launch.poll(context, true)?, expected);
        assert!(std::fs::OpenOptions::new().write(true).open(path).is_ok());
        Ok(())
    })
}
