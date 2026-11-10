// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! A capture the page delivered is local work: elapsed time never abandons it.
use super::*;
use sha2::{Digest, Sha256};

/// Two hours of injected time, far beyond any wall-clock bound a capture once had.
const LATER: Moment = Moment::from_nanos(7_200_000_000_000);

/// Cancel the startup restore so a close can request its final capture.
fn closing_with_final_capture(
    driver: &mut WorkspaceDriver,
    fence: SessionFence,
    cutoff: Moment,
) -> Result<SessionMessage, Box<dyn Error>> {
    let (restore, transfer) = encoding(ready(driver, fence)?)?;
    let cancel = driver.message(
        "00000000-0000-4000-8000-000000000098".into(),
        SessionOperation::Cancel { target_request_id: restore.clone() },
    );
    driver.receive(cancel, Moment::from_nanos(0))?;
    driver.encoding_stopped(&restore, &transfer)?;
    driver.begin_close(Moment::from_nanos(0), cutoff)?;
    driver.tick(Moment::from_nanos(0))?;
    Ok(driver.next_message().ok_or("Missing final capture")?)
}

#[test]
fn a_delivered_capture_is_decoded_and_published_after_any_elapsed_time() -> TestResult {
    let (mut driver, fence) = fixture()?;
    let mut checkpoint = driver
        .restored
        .as_ref()
        .and_then(|value| value.checkpoint.as_ref())
        .ok_or("Missing checkpoint")?
        .1
        .clone();
    let cutoff = LATER.checked_add(std::time::Duration::from_secs(3_600))?;
    let request = closing_with_final_capture(&mut driver, fence, cutoff)?;
    let SessionOperation::Capture { checkpoint_id, revision, .. } = request.operation else {
        return Err("Final capture was not requested".into());
    };
    // Waiting for the page is bounded by the capture's own cutoff, which wakes the pump.
    assert_eq!(driver.deadline(), Some(Moment::from_nanos(30_000_000_000)));
    checkpoint.id = checkpoint_id.clone();
    checkpoint.revision = revision;
    checkpoint.previous_id = None;
    for entry in &mut checkpoint.entries {
        entry.outputs.clear();
        entry.view.selection = None;
        entry.view.comparison.selection = None;
        entry.view.comparison.enabled = false;
        entry.view.cameras.clear();
        entry.view.navigation.batch_index = 0;
    }
    checkpoint.assets.clear();
    let bytes = sugarsubstitute_contracts::desktop_session::encode_checkpoint(&checkpoint)?;
    let transfer_id = "00000000-0000-4000-8000-000000000097".to_owned();
    let metadata = SessionTransfer {
        id: transfer_id.clone(),
        checkpoint_id,
        revision,
        bytes: bytes.len() as u64,
        chunks: 1,
        digest: files::hex(&Sha256::digest(&bytes)),
        direction: "capture".into(),
    };
    for operation in [
        SessionOperation::Begin { transfer: metadata },
        SessionOperation::Chunk {
            transfer_id: transfer_id.clone(),
            index: 0,
            text: String::from_utf8(bytes)?,
        },
    ] {
        driver.receive(
            driver.message(request.request_id.clone(), operation),
            Moment::from_nanos(1),
        )?;
    }
    let event = driver.receive(
        driver.message(
            request.request_id.clone(),
            SessionOperation::Commit { transfer_id: transfer_id.clone() },
        ),
        Moment::from_nanos(2),
    )?;
    let Some(DriverEvent::DecodeCapture { request_id, ticket, transfer, .. }) = event else {
        return Err("Final transfer did not request decoding".into());
    };
    // Decoding and publishing are the host's own jobs: no capture cutoff remains, and only the
    // application's shutdown cutoff bounds them and wakes the pump.
    assert_eq!(driver.deadline(), Some(cutoff));
    assert_eq!(driver.shutdown_cutoff(), Some(cutoff));
    driver.tick(LATER)?;
    let mut captured = transfer.finish(&transfer_id, Moment::from_nanos(3))?;
    driver.decoded_capture(&request_id, &ticket, &mut captured, LATER)?;
    driver.publishing(&ticket, &captured)?;
    let receipt = SaveReceipt {
        checkpoint_id: ticket.checkpoint.as_str().into(),
        revision: ticket.revision,
        bytes: 1,
        kept_previous: true,
        evidence: vec![],
    };
    driver.published(&ticket, captured, &receipt, LATER)?;
    let delivery = driver.next_message().ok_or("Durable delivery missing")?;
    assert!(
        matches!(delivery.operation, SessionOperation::Acknowledged { ref phase, .. } if phase == "durable")
    );
    Ok(())
}

#[test]
fn a_page_that_never_delivers_is_bounded_by_the_capture_cutoff_alone() -> TestResult {
    let (mut driver, fence) = fixture()?;
    closing_with_final_capture(&mut driver, fence, LATER)?;
    assert_eq!(driver.deadline(), Some(Moment::from_nanos(30_000_000_000)));
    driver.tick(Moment::from_nanos(31_000_000_000))?;
    let told = driver.next_message().ok_or("The page was not told to cancel")?;
    assert!(matches!(told.operation, SessionOperation::Cancel { .. }), "{:?}", told.operation);
    assert_ne!(driver.deadline(), Some(Moment::from_nanos(30_000_000_000)));
    Ok(())
}
