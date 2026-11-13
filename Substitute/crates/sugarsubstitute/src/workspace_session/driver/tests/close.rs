// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain final peer lifetime through actual durable acknowledgement delivery.
use super::*;
#[test]
fn final_close_waits_for_exact_durable_delivery_receipt_and_keeps_host_revision_chain() -> TestResult
{
    use sha2::{Digest, Sha256};
    let (mut driver, fence) = fixture()?;
    let mut checkpoint = driver
        .restored
        .as_ref()
        .and_then(|value| value.checkpoint.as_ref())
        .ok_or("Missing checkpoint")?
        .1
        .clone();
    let previous = checkpoint.id.clone();
    #[cfg(windows)]
    let retired = {
        let entry = checkpoint.entries.first().ok_or("Missing original entry")?;
        let output = entry.outputs.first().ok_or("Missing original output")?;
        let binding = fence
            .bindings
            .iter()
            .find(|binding| binding.entry_id == entry.id)
            .ok_or("Missing original runtime")?;
        let address = sugarsubstitute_contracts::canvas_surface::CanvasSurfaceAddress::decode(
            &serde_json::to_vec(
                &serde_json::json!({"sessionId":format!("canvas:{}",binding.runtime_session),"viewId":"view:test","windowId":"window:test","attachmentId":"attachment:test","generation":1}),
            )?,
        )?;
        let file = sugarsubstitute_contracts::canvas_surface::CanvasFileReference::decode(
            &serde_json::to_vec(
                &serde_json::json!({"filename":output.file.filename,"subfolder":output.file.subfolder,"type":output.file.storage}),
            )?,
        )?;
        (output.id.clone(), address, file)
    };
    let (restore, transfer) = encoding(ready(&mut driver, fence)?)?;
    let cancel = driver.message(
        "00000000-0000-4000-8000-000000000098".into(),
        SessionOperation::Cancel { target_request_id: restore.clone() },
    );
    driver.receive(cancel, Moment::from_nanos(0))?;
    driver.encoding_stopped(&restore, &transfer)?;
    driver.begin_close(Moment::from_nanos(0), Moment::from_nanos(10_000_000_000))?;
    driver.tick(Moment::from_nanos(0))?;
    let request = driver.next_message().ok_or("Missing final capture")?;
    let SessionOperation::Capture { checkpoint_id, revision, .. } = request.operation else {
        return Err("Final capture was not requested".into());
    };
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
    let mut captured = transfer.finish(&transfer_id, Moment::from_nanos(3))?;
    driver.decoded_capture(&request_id, &ticket, &mut captured, Moment::from_nanos(4))?;
    assert_eq!(captured.checkpoint.previous_id.as_deref(), Some(previous.as_str()));
    assert!(!driver.close_ready(Moment::from_nanos(5)));
    driver.publishing(&ticket, &captured)?;
    let receipt = SaveReceipt {
        checkpoint_id: ticket.checkpoint.as_str().into(),
        revision: ticket.revision,
        bytes: 1,
        kept_previous: true,
        evidence: vec![],
    };
    driver.published(&ticket, captured, &receipt, Moment::from_nanos(6))?;
    #[cfg(windows)]
    {
        assert!(driver.requires_verified_original(&retired.0));
        assert_eq!(
            driver.resolve_native_original(&retired.0, &retired.1, &retired.2),
            crate::workspace_session::NativeOriginalResolution::Unavailable
        );
    }
    assert!(!driver.close_ready(Moment::from_nanos(7)));
    let delivery = driver.next_message().ok_or("Durable delivery missing")?;
    let mut late_capture = delivery.clone();
    let SessionOperation::Acknowledged { phase, .. } = &mut late_capture.operation else {
        return Err("Wrong durable delivery operation".into());
    };
    *phase = "captured".into();
    let expected_ack = driver.last_ack.clone();
    for field in 0..4 {
        let mut foreign = late_capture.clone();
        let SessionOperation::Acknowledged { checkpoint_id, revision, bindings, .. } =
            &mut foreign.operation
        else {
            return Err("Wrong late capture operation".into());
        };
        match field {
            0 => foreign.request_id = "00000000-0000-4000-8000-000000000096".into(),
            1 => *checkpoint_id = "00000000-0000-4000-8000-000000000095".into(),
            2 => *revision += 1,
            _ => bindings.first_mut().ok_or("Missing captured binding")?.generation += 1,
        }
        assert!(matches!(
            driver.receive(foreign, Moment::from_nanos(8)),
            Err(StoreError::Contract(SessionError::Stale))
        ));
        assert_eq!(driver.last_ack, expected_ack);
        assert!(!driver.close_ready(Moment::from_nanos(8)));
        assert!(driver.next_message().is_none());
    }
    assert!(driver.receive(late_capture, Moment::from_nanos(9))?.is_none());
    assert_eq!(driver.last_ack, expected_ack);
    assert!(!driver.close_ready(Moment::from_nanos(9)));
    assert!(driver.next_message().is_none());
    let mut foreign = delivery.clone();
    foreign.request_id = "00000000-0000-4000-8000-000000000096".into();
    assert!(driver.receive(foreign, Moment::from_nanos(10)).is_err());
    assert!(!driver.close_ready(Moment::from_nanos(10)));
    driver.receive(delivery, Moment::from_nanos(11))?;
    assert!(driver.close_ready(Moment::from_nanos(11)));
    Ok(())
}
