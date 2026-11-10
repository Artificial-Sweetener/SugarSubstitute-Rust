// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe real cleanup jobs through repeated close and failed recovery completion.
use super::*;
use crate::{
    completion_wake::{Gate, Wake},
    paused_clock::PausedClock,
};
use std::sync::Mutex;

fn fixture() -> Result<(Controller, Arc<Wake>)> {
    let wake = Arc::new(Wake::default());
    let mut controller = Controller::new(PathBuf::new(), PathBuf::new(), &[]);
    controller.clock = PausedClock::new();
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

/// Re-examine `step` after each execution wake; slow real work only makes the observation longer.
fn observe(wake: &Wake, mut step: impl FnMut() -> Result<bool>) -> Result<()> {
    wake.until(|| step().map(|done| done.then_some(())).transpose())
}

/// Release the held worker when the test unwinds, so a failed assertion cannot strand its join.
struct Release(Arc<Gate>);
impl Drop for Release {
    fn drop(&mut self) {
        self.0.release();
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
        controller.advance_events()?;
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
        for message in controller.advance_events()?.events {
            if message.id.as_deref() == Some("open") && matches!(message.kind, Kind::Failed(_)) {
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
        controller.advance_events()?;
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
            messages.extend(controller.advance_events()?.events);
            Ok(controller.relaunch.is_none())
        })?;
        let products: Vec<_> =
            messages.iter().filter(|event| !matches!(event.kind, Kind::Diagnostic(_))).collect();
        let diagnostics: Vec<_> = messages
            .iter()
            .filter_map(|event| match &event.kind {
                Kind::Diagnostic(data) => Some(data),
                _ => None,
            })
            .collect();
        assert_eq!(diagnostics.len(), usize::from(creation != CreationOutcome::Created));
        assert!(
            diagnostics
                .iter()
                .all(|data| data.message == "original worker outcome" && data.omitted == 0)
        );
        assert_eq!(products.len(), 1);
        assert!(matches!(
            (&products[0].kind, creation),
            (Kind::Failed(_), CreationOutcome::NotCreated)
                | (Kind::Completed(Completed::Opened), CreationOutcome::Created)
                | (Kind::Unavailable(_), CreationOutcome::Indeterminate)
        ));
        assert_eq!(controller.unavailable, creation == CreationOutcome::Indeterminate);
        assert!(controller.install_id.is_none());
        assert!(controller.launch_failure.is_none());
        controller.close()?;
        observe(&wake, || {
            controller.advance_events()?;
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
            assert!(controller.advance_events()?.events.is_empty());
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
    observe(&wake, || match controller.advance_events().map(|batch| batch.events) {
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
    let mut diagnostics = 0;
    observe(&wake, || {
        for message in controller.advance_events()?.events {
            if let Kind::Diagnostic(data) = &message.kind {
                assert!(message.id.is_none());
                assert_eq!(data.message, "original launch failure");
                diagnostics += 1;
                continue;
            }
            assert_eq!(message.id.as_deref(), Some("open"));
            assert!(
                matches!(message.kind, Kind::Failed(ref cause) if cause == sugarsubstitute_contracts::localization::installer::messages("en").setup_failed)
            );
            reports += 1;
        }
        Ok(controller.relaunch.is_none())
    })?;
    assert_eq!(reports, 1);
    assert_eq!(diagnostics, 1);
    assert!(controller.install_id.is_none());
    assert!(controller.launch_failure.is_none());
    controller.close()?;
    observe(&wake, || {
        controller.advance_events()?;
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
        observe(&wake, || match controller.advance_events().map(|batch| batch.events) {
            Err(cause) => {
                assert_eq!(cause.to_string(), "original recovery failure");
                Ok(true)
            }
            Ok(_) => Ok(false),
        })?;
        assert!(controller.relaunch.is_some());
        assert!(controller.storage.is_none());
        observe(&wake, || {
            controller.advance_events()?;
            controller.settled()
        })?;
        controller.finish()?;
    }
    Ok(())
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
        let gate = Arc::new(Gate::default());
        let _release = Release(gate.clone());
        let cancelled = Arc::new(Mutex::new(None));
        let (worker_gate, worker_cancelled) = (gate.clone(), cancelled.clone());
        controller.submit(work, move |context| {
            worker_gate.hold();
            *worker_cancelled.lock().map_err(|_| io::Error::other("Boundary poisoned"))? =
                Some(context.cancellation_requested());
            Ok(StorageResult::Launched)
        })?;
        gate.wait_entered();
        controller.close()?;
        assert!(!controller.settled()?);
        gate.release();
        observe(&wake, || {
            controller.advance_events()?;
            controller.settled()
        })?;
        assert_eq!(*cancelled.lock().map_err(|_| "Boundary poisoned")?, Some(cancel));
        controller.finish()?;
    }
    Ok(())
}

#[test]
fn close_during_open_retains_returned_creator_until_observed_retirement() -> Result<()> {
    let (mut controller, wake) = fixture()?;
    let launch = controller.relaunch.take().ok_or("Creator absent")?;
    controller.install_id = Some("open".into());
    let gate = Arc::new(Gate::default());
    let _release = Release(gate.clone());
    let worker_gate = gate.clone();
    controller.submit(StorageWork::Open, move |_| {
        worker_gate.hold();
        Ok(StorageResult::Opened(super::super::completion::OpenedLaunch::Installed(launch)))
    })?;
    gate.wait_entered();
    controller.close()?;
    assert!(controller.relaunch.is_none());
    gate.release();
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
        assert!(controller.advance_events()?.events.is_empty());
        controller.settled()
    })?;
    assert!(controller.relaunch.is_none());
    controller.finish()?;
    Ok(())
}
