// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe real cleanup jobs through repeated close and failed recovery completion.
use super::*;
use std::{
    sync::{Condvar, Mutex},
    time::Instant,
};
use sugarsubstitute_contracts::execution::ExecutionError;

#[derive(Default)]
struct Wake(Mutex<u64>, Condvar);
impl CompletionWake for Wake {
    fn wake(&self) -> std::result::Result<(), ExecutionError> {
        let mut value = self.0.lock().map_err(|_| {
            ExecutionError::new(
                Operation::Infrastructure,
                Box::new(io::Error::other("Cleanup observer poisoned")),
            )
        })?;
        *value += 1;
        self.1.notify_all();
        Ok(())
    }
}

fn fixture() -> Result<(Controller, Arc<Wake>)> {
    let wake = Arc::new(Wake::default());
    let mut controller = Controller::new(PathBuf::new(), PathBuf::new(), &[]);
    let owner = ExecutionOwner::new(controller.clock.clone(), wake.clone())?;
    let client = owner.client();
    controller.scope =
        Some(client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?);
    controller.owner = Some(owner);
    controller.client = Some(client);
    controller.relaunch =
        Some(ApplicationLaunch::new(sugarsubstitute_supervision::WindowsLaunchCommand::from_os(
            std::path::Path::new(r"E:\unopened\app.exe"),
            std::path::Path::new(r"E:\unopened"),
            &[],
        )?));
    Ok((controller, wake))
}

fn observe(wake: &Wake, mut step: impl FnMut() -> Result<bool>) -> Result<()> {
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        let revision = *wake.0.lock().map_err(|_| "Cleanup observer poisoned")?;
        if step()? {
            return Ok(());
        }
        let remaining =
            until.checked_duration_since(Instant::now()).ok_or("Cleanup observer expired")?;
        let (guard, timed) = wake
            .1
            .wait_timeout_while(
                wake.0.lock().map_err(|_| "Cleanup observer poisoned")?,
                remaining,
                |current| *current == revision,
            )
            .map_err(|_| "Cleanup observer poisoned")?;
        drop(guard);
        if timed.timed_out() {
            // Worker return can follow its last completion wake. Inspect actual settlement
            // before treating an expired observation as an unfinished operation.
            if step()? {
                return Ok(());
            }
            return Err("Cleanup observer expired".into());
        }
    }
}

#[test]
fn repeated_close_preserves_admitted_launch_cleanup_until_observed_settlement() -> Result<()> {
    let (mut controller, wake) = fixture()?;
    controller.close()?;
    assert!(!controller.settled()?);
    assert_eq!(
        controller.storage.as_ref().ok_or("Cleanup job absent")?.job.spec().class,
        JobClass::InfrastructureLifecycle
    );
    controller.close()?;
    observe(&wake, || {
        controller.advance()?;
        controller.settled()
    })?;
    assert!(controller.relaunch.is_none());
    controller.finish()?;
    assert!(controller.owner.is_none());
    Ok(())
}

#[test]
fn failed_transfer_recovers_creator_custody_while_the_installer_stays_open() -> Result<()> {
    let (mut controller, wake) = fixture()?;
    controller.install_id = Some("open".into());
    controller.submit(StorageWork::Transfer, |_| Err(io::Error::other("transfer failure")))?;
    let mut reported = false;
    observe(&wake, || {
        for message in controller.advance()? {
            let message: serde_json::Value = serde_json::from_str(&message)?;
            if message["id"] == "open" && message["event"] == "failed" {
                assert!(controller.relaunch.is_none());
                assert!(controller.launch_failure.is_none());
                assert!(controller.install_id.is_none());
                reported = true;
            }
        }
        Ok(controller.relaunch.is_none())
    })?;
    assert!(reported);
    assert!(!controller.closing);
    assert!(controller.storage.is_none());
    controller.close()?;
    observe(&wake, || {
        controller.advance()?;
        controller.settled()
    })?;
    controller.finish()?;
    Ok(())
}

#[test]
fn cleanup_uses_native_creation_fact_instead_of_the_discarded_worker_outcome() -> Result<()> {
    for creation in
        [CreationOutcome::NotCreated, CreationOutcome::Created, CreationOutcome::Indeterminate]
    {
        let (mut controller, wake) = fixture()?;
        controller.install_id = Some("open".into());
        controller.launch_failure = Some(io::Error::other("original worker outcome"));
        controller.submit(StorageWork::Retire, move |_| Ok(StorageResult::Retired(creation)))?;
        let mut messages = Vec::new();
        observe(&wake, || {
            messages.extend(controller.advance()?);
            Ok(controller.relaunch.is_none())
        })?;
        let messages: Vec<serde_json::Value> = messages
            .iter()
            .map(|message| serde_json::from_str(message))
            .collect::<std::result::Result<_, _>>()?;
        let product: Vec<_> =
            messages.iter().filter(|message| message["event"] != "diagnostic").collect();
        assert_eq!(product.len(), 1);
        assert_eq!(
            messages.iter().filter(|message| message["event"] == "diagnostic").count(),
            usize::from(creation != CreationOutcome::Created)
        );
        if creation != CreationOutcome::Created {
            assert_eq!(messages[0]["event"], "diagnostic");
            assert_eq!(messages[0]["data"]["message"], "original worker outcome");
        }
        let message = product[0];
        assert_eq!(
            message["event"],
            match creation {
                CreationOutcome::NotCreated => "failed",
                CreationOutcome::Created => "completed",
                CreationOutcome::Indeterminate => "unavailable",
            }
        );
        assert_eq!(controller.unavailable, creation == CreationOutcome::Indeterminate);
        assert!(controller.install_id.is_none());
        assert!(controller.launch_failure.is_none());
        controller.close()?;
        observe(&wake, || {
            controller.advance()?;
            controller.settled()
        })?;
        controller.finish()?;
    }
    Ok(())
}

#[test]
fn closing_observes_creation_cleanup_without_sending_a_second_open_response() -> Result<()> {
    for creation in [CreationOutcome::Created, CreationOutcome::Indeterminate] {
        let (mut controller, wake) = fixture()?;
        controller.install_id = Some("open".into());
        controller.launch_failure = Some(io::Error::other("discarded worker outcome"));
        controller.submit(StorageWork::Retire, move |_| Ok(StorageResult::Retired(creation)))?;
        controller.close()?;
        observe(&wake, || {
            assert!(controller.advance()?.is_empty());
            controller.settled()
        })?;
        assert!(controller.relaunch.is_none());
        assert!(controller.launch_failure.is_none());
        controller.finish()?;
    }
    Ok(())
}

#[test]
fn recovery_failure_retains_the_original_open_request_until_cleanup_succeeds() -> Result<()> {
    let (mut controller, wake) = fixture()?;
    controller.install_id = Some("open".into());
    controller.launch_failure = Some(io::Error::other("original launch failure"));
    controller.submit(StorageWork::Retire, |_| Err(io::Error::other("cleanup failure")))?;
    observe(&wake, || match controller.advance() {
        Err(cause) => {
            assert_eq!(cause.to_string(), "cleanup failure");
            Ok(true)
        }
        Ok(messages) => {
            assert!(messages.is_empty());
            Ok(false)
        }
    })?;
    assert_eq!(controller.install_id.as_deref(), Some("open"));
    assert_eq!(
        controller.launch_failure.as_ref().ok_or("Original failure lost")?.to_string(),
        "original launch failure"
    );
    let launch = controller.relaunch.as_ref().ok_or("Creator lost")?.clone();
    controller.submit(StorageWork::Retire, move |context| {
        launch.recover_and_retire(&context).map(StorageResult::Retired).map_err(io::Error::other)
    })?;
    let mut reports = 0;
    observe(&wake, || {
        for message in controller.advance()? {
            let message: serde_json::Value = serde_json::from_str(&message)?;
            if message["event"] == "diagnostic" {
                assert_eq!(message["data"]["message"], "original launch failure");
                continue;
            }
            assert_eq!(message["id"], "open");
            assert_eq!(message["event"], "failed");
            assert_eq!(
                message["data"]["message"],
                sugarsubstitute_contracts::localization::installer::messages("en").setup_failed
            );
            reports += 1;
        }
        Ok(controller.relaunch.is_none())
    })?;
    assert_eq!(reports, 1);
    assert!(controller.install_id.is_none());
    assert!(controller.launch_failure.is_none());
    controller.close()?;
    observe(&wake, || {
        controller.advance()?;
        controller.settled()
    })?;
    controller.finish()?;
    Ok(())
}

#[test]
fn closing_does_not_swallow_cleanup_failure_or_drop_retained_launch_ownership() -> Result<()> {
    for work in [StorageWork::Retire, StorageWork::Transfer, StorageWork::Launch] {
        let (mut controller, wake) = fixture()?;
        controller.submit(work, |_| Err(io::Error::other("original recovery failure")))?;
        controller.close()?;
        observe(&wake, || match controller.advance() {
            Err(cause) => {
                assert_eq!(cause.to_string(), "original recovery failure");
                Ok(true)
            }
            Ok(_) => Ok(false),
        })?;
        assert!(controller.relaunch.is_some());
        assert!(controller.storage.is_none());
        observe(&wake, || {
            controller.advance()?;
            controller.settled()
        })?;
        controller.finish()?;
    }
    Ok(())
}

#[derive(Default)]
struct Boundary {
    entered: bool,
    release: bool,
    cancelled: Option<bool>,
}

#[test]
fn closing_cancels_preparation_but_observes_transfer_and_creation_without_discarding_results()
-> Result<()> {
    for (work, cancel) in [
        (StorageWork::Metadata, true),
        (StorageWork::Publish, true),
        (StorageWork::Open, false),
        (StorageWork::Transfer, false),
        (StorageWork::Launch, false),
    ] {
        let (mut controller, wake) = fixture()?;
        let boundary = Arc::new((Mutex::new(Boundary::default()), Condvar::new()));
        let worker_boundary = boundary.clone();
        let worker_wake = wake.clone();
        controller.submit(work, move |context| {
            let mut state =
                worker_boundary.0.lock().map_err(|_| io::Error::other("Boundary poisoned"))?;
            state.entered = true;
            worker_wake.wake().map_err(io::Error::other)?;
            let (mut state, timeout) = worker_boundary
                .1
                .wait_timeout_while(state, Duration::from_secs(10), |state| !state.release)
                .map_err(|_| io::Error::other("Boundary poisoned"))?;
            if timeout.timed_out() {
                return Err(io::Error::other("Boundary observer expired"));
            }
            state.cancelled = Some(context.cancellation_requested());
            Ok(StorageResult::Launched)
        })?;
        observe(&wake, || Ok(boundary.0.lock().map_err(|_| "Boundary poisoned")?.entered))?;
        controller.close()?;
        assert!(!controller.settled()?);
        boundary.0.lock().map_err(|_| "Boundary poisoned")?.release = true;
        boundary.1.notify_all();
        observe(&wake, || {
            controller.advance()?;
            controller.settled()
        })?;
        assert_eq!(boundary.0.lock().map_err(|_| "Boundary poisoned")?.cancelled, Some(cancel));
        controller.finish()?;
    }
    Ok(())
}

#[test]
fn production_payload_import_and_preparation_jobs_authorize_protected_cache_creation() -> Result<()>
{
    let (mut controller, wake) = fixture()?;
    controller.relaunch = None;
    for work in
        [StorageWork::Payload, StorageWork::Import, StorageWork::Prepare, StorageWork::Publish]
    {
        let root = tempfile::tempdir()?;
        let path = root.path().to_owned();
        controller.submit(work, move |context| {
            assert!(context.deadline().is_some());
            let cache = setup_storage::cache::RuntimeCache::open(
                &path,
                setup_storage::cache::CachePolicy::default(),
                &context,
            )?;
            assert!(cache.path().join("owner").is_file());
            Ok(StorageResult::Transferred)
        })?;
        observe(&wake, || {
            controller
                .client
                .as_ref()
                .ok_or("Execution client absent")?
                .drain_completions(NonZeroUsize::new(32).ok_or("Invalid drain bound")?)?;
            let job = &mut controller.storage.as_mut().ok_or("Storage job absent")?.job;
            let Some(completion) = job.try_take_completion() else {
                return Ok(false);
            };
            controller.storage = None;
            match completion.outcome {
                JobOutcome::Success(StorageResult::Transferred) => Ok(true),
                JobOutcome::Failure(cause) => Err(cause.into()),
                _ => Err("Protected cache creation did not complete".into()),
            }
        })?;
        assert!(root.path().join(setup_storage::cache::DIRECTORY).join("owner").is_file());
    }
    controller.close()?;
    observe(&wake, || {
        controller.advance()?;
        controller.settled()
    })?;
    controller.finish()
}

#[test]
fn close_during_open_retains_returned_creator_until_observed_retirement() -> Result<()> {
    let (mut controller, wake) = fixture()?;
    let launch = controller.relaunch.take().ok_or("Creator absent")?;
    controller.install_id = Some("open".into());
    let boundary = Arc::new((Mutex::new(Boundary::default()), Condvar::new()));
    let worker_boundary = boundary.clone();
    let worker_wake = wake.clone();
    controller.submit(StorageWork::Open, move |_| {
        let mut state =
            worker_boundary.0.lock().map_err(|_| io::Error::other("Boundary poisoned"))?;
        state.entered = true;
        worker_wake.wake().map_err(io::Error::other)?;
        let (_state, timeout) = worker_boundary
            .1
            .wait_timeout_while(state, Duration::from_secs(10), |state| !state.release)
            .map_err(|_| io::Error::other("Boundary poisoned"))?;
        if timeout.timed_out() {
            return Err(io::Error::other("Boundary observer expired"));
        }
        Ok(StorageResult::Opened(super::super::completion::OpenedLaunch::Installed(launch)))
    })?;
    observe(&wake, || Ok(boundary.0.lock().map_err(|_| "Boundary poisoned")?.entered))?;
    controller.close()?;
    assert!(controller.relaunch.is_none());
    boundary.0.lock().map_err(|_| "Boundary poisoned")?.release = true;
    boundary.1.notify_all();
    observe(&wake, || {
        controller.poll_storage()?;
        Ok(controller.storage.is_none())
    })?;
    assert!(controller.relaunch.is_some());
    assert!(!controller.settled()?);
    assert!(matches!(
        controller.storage.as_ref().ok_or("Recovery absent")?.work,
        StorageWork::Retire
    ));
    observe(&wake, || {
        assert!(controller.advance()?.is_empty());
        controller.settled()
    })?;
    assert!(controller.relaunch.is_none());
    controller.finish()?;
    Ok(())
}
