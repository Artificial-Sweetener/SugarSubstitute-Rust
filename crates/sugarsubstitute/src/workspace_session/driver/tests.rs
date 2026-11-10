// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Protect authored startup intent, original replay fences and observed encoding retirement.
mod capture;
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
    driver.restore_failed(&request, &RestoreFailure::Encoding)?;
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
#[test]
fn a_restore_waiting_on_its_page_is_never_abandoned_by_elapsed_time() -> TestResult {
    let (mut driver, fence) = fixture()?;
    let (request, _) = encoding(ready(&mut driver, fence)?)?;
    // The restore has no cutoff of its own to wake the pump for.
    assert!(driver.deadline().is_none());
    for seconds in [30, 31, 600, 86_400, 31_536_000] {
        driver.tick(Moment::from_nanos(seconds * 1_000_000_000))?;
        assert!(driver.owns_restore(&request), "restore abandoned after {seconds} s");
        assert!(driver.next_message().is_none());
    }
    Ok(())
}
#[test]
fn a_page_refusal_ends_the_restore_with_a_typed_cause_and_protects_the_saved_workspace()
-> TestResult {
    for code in ["invalid", "unavailable", "not-committed"] {
        let (mut driver, fence) = fixture()?;
        let (request, transfer) = encoding(ready(&mut driver, fence)?)?;
        driver.encoding_stopped(&request, &transfer)?;
        driver.restore.as_mut().ok_or("Restore disappeared")?.sent = true;
        let refusal = driver.message(
            request.clone(),
            SessionOperation::Error {
                failure: SessionFailure {
                    code: code.into(),
                    retryable: code != "invalid",
                    entry_id: None,
                    output_id: None,
                },
            },
        );
        assert!(driver.receive(refusal, Moment::from_nanos(1))?.is_none());
        assert!(!driver.owns_restore(&request), "{code} left the restore owned");
        assert_eq!(
            RestoreFailure::Refused { code: code.into(), retryable: code != "invalid" }.to_string(),
            format!("page-refused code={code} retryable={}", code != "invalid")
        );
        // The saved workspace is protected: no default workspace is ever captured over it.
        driver.tick(Moment::from_nanos(31_000_000_000))?;
        assert!(driver.next_message().is_none());
        assert!(driver.policy.deadline().is_none());
    }
    Ok(())
}
#[test]
fn closing_ends_a_restore_still_waiting_on_its_page_and_protects_the_saved_workspace() -> TestResult
{
    let (mut driver, fence) = fixture()?;
    let (request, transfer) = encoding(ready(&mut driver, fence)?)?;
    driver.encoding_stopped(&request, &transfer)?;
    driver.begin_close(Moment::from_nanos(5), Moment::from_nanos(10_000_000_000))?;
    assert!(!driver.owns_restore(&request));
    driver.tick(Moment::from_nanos(10_000_000_000))?;
    assert!(driver.close_ready(Moment::from_nanos(10_000_000_000)));
    assert!(driver.next_message().is_none());
    assert_eq!(RestoreFailure::Shutdown.to_string(), "shutdown");
    Ok(())
}
