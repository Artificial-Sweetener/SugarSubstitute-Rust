// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Gate the exit intent on the close save and record what it did not keep.
use super::closing_tests::Fixture;
use super::*;
use sha2::{Digest, Sha256};
use sugarsubstitute_contracts::lifecycle::ShutdownBudget;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;
use crate::startup::close_save::InputSaveProbe;
use sugarsubstitute_application::workspace_session::close_save::{
    CloseReason, CloseSaveOutcome, CloseSaveProgress, NotSavedLastTime, SaveFailure, SaveVerdict,
    UnsavedCause, UnsavedItem,
};

#[test]
fn the_input_final_save_is_submitted_before_the_exit_intent_and_its_failure_is_recorded()
-> TestResult {
    let mut fixture = Fixture::new(true)?;
    // No workspace was loaded, so only the input canvas has work to keep.
    fixture.host.workspace.as_mut().ok_or("Workspace")?.driver = None;
    fixture.host.input_probe = Some(InputSaveProbe::default());
    fixture.host.request_close(CloseReason::RestartToApply)?;
    let budget = fixture.host.shutdown.budget()?;
    fixture.poll()?;
    let probe = fixture.host.input_probe.as_ref().ok_or("Probe")?;
    assert_eq!(probe.begun, Some(budget.graceful_until), "the input save began first");
    assert!(fixture.host.child.shutdown_requests.is_empty(), "no exit intent before its answer");
    assert!(matches!(
        fixture.host.close_save_progress(),
        Some(CloseSaveProgress::Saving { reason: CloseReason::RestartToApply, .. })
    ));
    assert!(fixture.host.close_save_outcome().is_none());
    // Fault injection: the input store write failed.
    fixture.host.input_probe.as_mut().ok_or("Probe")?.verdict =
        Some(SaveVerdict::Failed(SaveFailure::Storage));
    assert_eq!(fixture.exit_intent()?, budget);
    assert_eq!(
        fixture.host.close_save_outcome(),
        Some(CloseSaveOutcome {
            reason: CloseReason::RestartToApply,
            workspace: SaveVerdict::Saved,
            inputs: SaveVerdict::Failed(SaveFailure::Storage),
        })
    );
    assert_eq!(
        fixture.next_start_notice()?,
        Some(NotSavedLastTime {
            reason: CloseReason::RestartToApply,
            items: vec![UnsavedItem::InputCanvas(UnsavedCause::Failed(SaveFailure::Storage))],
        })
    );
    assert_eq!(fixture.next_start_notice()?, None, "the notice appears once");
    fixture.retire()
}

#[test]
fn a_saved_close_writes_no_record() -> TestResult {
    let mut fixture = Fixture::new(true)?;
    fixture.host.workspace.as_mut().ok_or("Workspace")?.driver = None;
    fixture.host.observe(DesktopStartupObservation::CloseRequested)?;
    fixture.exit_intent()?;
    assert!(fixture.host.close_save_outcome().is_some_and(|outcome| outcome.saved()));
    assert_eq!(fixture.next_start_notice()?, None);
    let record = fixture.host.workspace.as_ref().and_then(|work| work.record_directory.clone());
    assert!(!record.ok_or("Record directory")?.join("close-save.json").exists());
    fixture.retire()
}

#[test]
fn passing_the_graceful_cutoff_before_the_final_capture_records_the_workspace_as_stale()
-> TestResult {
    let mut fixture = Fixture::new(true)?;
    fixture.host.observe(DesktopStartupObservation::CloseRequested)?;
    let original = fixture.host.shutdown.budget()?;
    fixture.sample()?;
    fixture.poll()?;
    // The final capture is requested; the page never delivers it.
    assert!(matches!(fixture.host.take_command()?, Some(DesktopStartupCommand::SendSession(_))));
    assert!(fixture.host.child.shutdown_requests.is_empty());
    // The clock passes the graceful cutoff: an earlier observed budget whose grace is over.
    let started = Moment::from_nanos(original.started.as_nanos().saturating_sub(1));
    let cutoff = ShutdownBudget { graceful_until: started, ..ShutdownBudget::new(started)? };
    cutoff.validate()?;
    fixture.host.child.budget = Some(cutoff);
    assert_eq!(fixture.exit_intent()?, cutoff);
    let outcome = fixture.host.close_save_outcome().ok_or("Outcome")?;
    assert!(matches!(outcome.workspace, SaveVerdict::Stale { .. }), "{outcome:?}");
    assert_eq!(outcome.inputs, SaveVerdict::Saved);
    let notice = fixture.next_start_notice()?.ok_or("Notice")?;
    assert!(matches!(
        notice.items.as_slice(),
        [UnsavedItem::Workspace(UnsavedCause::Interrupted { .. })]
    ));
    fixture.retire()
}

#[test]
fn a_failed_final_workspace_publication_is_recorded_before_the_exit_intent() -> TestResult {
    let mut fixture = Fixture::new(true)?;
    fixture.host.observe(DesktopStartupObservation::CloseRequested)?;
    fixture.sample()?;
    fixture.poll()?;
    let Some(DesktopStartupCommand::SendSession(command)) = fixture.host.take_command()? else {
        return Err("Final capture absent".into());
    };
    let request = command.message().clone();
    let SessionOperation::Capture { checkpoint_id, revision, .. } = &request.operation else {
        return Err("Wrong final operation".into());
    };
    fixture.checkpoint.id = checkpoint_id.clone();
    fixture.checkpoint.revision = *revision;
    fixture.checkpoint.previous_id = None;
    let bytes = sugarsubstitute_contracts::desktop_session::encode_checkpoint(&fixture.checkpoint)?;
    let transfer_id = "00000000-0000-4000-8000-000000000097".to_owned();
    let metadata = SessionTransfer {
        id: transfer_id.clone(),
        checkpoint_id: checkpoint_id.clone(),
        revision: *revision,
        bytes: bytes.len() as u64,
        chunks: 1,
        digest: format!("{:x}", Sha256::digest(&bytes)),
        direction: "capture".into(),
    };
    let mut message = request.clone();
    let now = fixture.host.clock.now();
    let driver =
        fixture.host.workspace.as_mut().and_then(|work| work.driver.as_mut()).ok_or("Driver")?;
    for operation in [
        SessionOperation::Begin { transfer: metadata },
        SessionOperation::Chunk {
            transfer_id: transfer_id.clone(),
            index: 0,
            text: String::from_utf8(bytes)?,
        },
    ] {
        message.operation = operation;
        assert!(driver.receive(message.clone(), now)?.is_none());
    }
    message.operation = SessionOperation::Commit { transfer_id: transfer_id.clone() };
    let Some(DriverEvent::DecodeCapture { request_id, ticket, transfer, .. }) =
        driver.receive(message, now)?
    else {
        return Err("Final decode absent".into());
    };
    let mut captured = transfer.finish(&transfer_id, now)?;
    driver.decoded_capture(&request_id, &ticket, &mut captured, now)?;
    driver.publishing(&ticket, &captured)?;
    // Fault injection at the publication's store write: its directory is a file.
    let checkpoints = fixture.directory.path().join("checkpoints");
    if checkpoints.is_dir() {
        std::fs::remove_dir_all(&checkpoints)?;
    }
    std::fs::write(&checkpoints, b"not a directory")?;
    assert!(fixture.save(captured.checkpoint.clone()).is_err(), "the store write fails");
    let now = fixture.host.clock.now();
    fixture
        .host
        .workspace
        .as_mut()
        .and_then(|work| work.driver.as_mut())
        .ok_or("Driver")?
        .publication_failed(&ticket, now)?;
    fixture.exit_intent()?;
    let outcome = fixture.host.close_save_outcome().ok_or("Outcome")?;
    assert_eq!(outcome.workspace, SaveVerdict::Failed(SaveFailure::Storage));
    let notice = fixture.next_start_notice()?.ok_or("Notice")?;
    assert_eq!(
        notice.items,
        vec![UnsavedItem::Workspace(UnsavedCause::Failed(SaveFailure::Storage))]
    );
    fixture.retire()
}
