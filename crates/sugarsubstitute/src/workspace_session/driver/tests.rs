// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Protect authored startup intent, original replay fences and observed encoding retirement.
mod close;
mod media;
use super::*;
use sugarsubstitute_contracts::execution::MemoryRelease;
type TestResult = Result<(), Box<dyn Error>>;
struct Release;
impl MemoryRelease for Release {
    fn release(&self, _: u64) {}
}
fn fixture() -> Result<(WorkspaceDriver, SessionFence), Box<dyn Error>> {
    let vectors: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../third_party/sugarcubes-session/session_vectors.json"
    ))?;
    let checkpoint = sugarsubstitute_contracts::desktop_session::decode_checkpoint(
        &serde_json::to_vec(&vectors["checkpoints"][0]["value"])?,
    )?;
    let mut fence: SessionFence = serde_json::from_value(vectors["currentFence"].clone())?;
    fence.revision = 1;
    let driver = WorkspaceDriver::new(
        checkpoint.scope.clone(),
        vectors["origin"].as_str().ok_or("Missing origin")?.into(),
        fence.webview.clone(),
        RestoredWorkspace {
            checkpoint: Some((CheckpointFile::Current, checkpoint)),
            rejected: vec![],
            protected: false,
            _memory: None,
        },
        MemoryLease::new(1024 * 1024 * 1024, Box::new(Release)),
    )?;
    Ok((driver, fence))
}
fn ready(
    driver: &mut WorkspaceDriver,
    fence: SessionFence,
) -> Result<Option<DriverEvent>, StoreError> {
    ready_state(driver, fence, true)
}
fn ready_state(
    driver: &mut WorkspaceDriver,
    fence: SessionFence,
    committed: bool,
) -> Result<Option<DriverEvent>, StoreError> {
    let operation = SessionOperation::Ready {
        fence,
        capabilities: SessionCapabilities {
            capture: true,
            materialize: true,
            chunk_transfer: true,
            artifact_resolution: true,
        },
        committed,
        workflow_persistence: true,
    };
    driver.receive(
        driver.message("00000000-0000-4000-8000-000000000099".into(), operation),
        Moment::from_nanos(0),
    )
}
fn encoding(event: Option<DriverEvent>) -> Result<(String, String), Box<dyn Error>> {
    match event {
        Some(DriverEvent::EncodeRestore { request_id, transfer_id, .. }) => {
            Ok((request_id, transfer_id))
        }
        _ => Err("Missing initial restore".into()),
    }
}
#[test]
fn authored_change_cancels_replay_and_waits_for_actual_encoding_terminal() -> TestResult {
    let (mut driver, mut fence) = fixture()?;
    let (request, transfer) = encoding(ready(&mut driver, fence.clone())?)?;
    fence.revision += 1;
    fence.bindings[0].graph_revision += 1;
    assert!(ready(&mut driver, fence)?.is_none());
    assert!(!driver.owns_restore(&request));
    assert!(
        matches!(driver.next_message().map(|message| message.operation), Some(SessionOperation::Cancel { target_request_id }) if target_request_id == request)
    );
    driver.tick(Moment::from_nanos(3_000_000_000))?;
    assert!(driver.next_message().is_none());
    driver.encoding_stopped(&request, &transfer)?;
    assert!(driver.encoding_stopped(&request, &transfer).is_err());
    driver.tick(Moment::from_nanos(3_000_000_000))?;
    assert!(matches!(
        driver.next_message().map(|message| message.operation),
        Some(SessionOperation::Capture { revision: 10, .. })
    ));
    Ok(())
}
#[test]
fn camera_observation_refreshes_only_the_same_authored_restore_frontier() -> TestResult {
    let (mut driver, mut fence) = fixture()?;
    let (request, _) = encoding(ready(&mut driver, fence.clone())?)?;
    fence.revision += 1;
    fence.bindings[0].navigation_revision += 1;
    fence.bindings[1].source_revision += 1;
    assert!(ready(&mut driver, fence.clone())?.is_none());
    assert!(driver.owns_restore(&request));
    assert_eq!(driver.restore.as_ref().map(|restore| &restore.fence), Some(&fence));
    Ok(())
}
#[test]
fn controlled_materialize_bindings_do_not_retarget_the_original_request() -> TestResult {
    let (mut driver, mut fence) = fixture()?;
    let (request, _) = encoding(ready(&mut driver, fence.clone())?)?;
    let original = fence.clone();
    driver.restore.as_mut().ok_or("Restore disappeared")?.sent = true;
    fence.revision += 1;
    fence.bindings[0].runtime_session = "restored-fresh-runtime".into();
    assert!(ready(&mut driver, fence)?.is_none());
    assert!(driver.owns_restore(&request));
    assert_eq!(driver.restore.as_ref().map(|restore| &restore.fence), Some(&original));
    let refusal = driver.message(
        request.clone(),
        SessionOperation::Error {
            failure: SessionFailure {
                code: "stale".into(),
                retryable: false,
                entry_id: None,
                output_id: None,
            },
        },
    );
    assert!(driver.receive(refusal, Moment::from_nanos(1))?.is_none());
    assert!(!driver.owns_restore(&request));
    Ok(())
}
#[test]
fn replacement_runtime_or_occurrence_cancels_pending_replay() -> TestResult {
    for replacement in [false, true] {
        let (mut driver, mut fence) = fixture()?;
        let (request, _) = encoding(ready(&mut driver, fence.clone())?)?;
        fence.revision += 1;
        if replacement {
            fence.bindings[0].generation += 1;
        } else {
            fence.bindings.pop();
        }
        assert!(ready(&mut driver, fence)?.is_none());
        assert!(!driver.owns_restore(&request));
    }
    Ok(())
}
#[test]
fn failed_replay_protects_originals_without_renewing_close_or_capturing_defaults() -> TestResult {
    let (mut driver, fence) = fixture()?;
    let (request, transfer) = encoding(ready(&mut driver, fence)?)?;
    driver.encoding_stopped(&request, &transfer)?;
    driver.tick(Moment::from_nanos(30_000_000_000))?;
    assert!(
        matches!(driver.next_message().map(|message| message.operation), Some(SessionOperation::Cancel { target_request_id }) if target_request_id == request)
    );
    driver.tick(Moment::from_nanos(31_000_000_000))?;
    assert!(driver.next_message().is_none());
    driver.begin_close(Moment::from_nanos(31_000_000_000), Moment::from_nanos(40_000_000_000))?;
    driver.tick(Moment::from_nanos(40_000_000_000))?;
    assert!(driver.close_ready(Moment::from_nanos(40_000_000_000)));
    assert!(driver.next_message().is_none());
    Ok(())
}
