// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Workspace work no executor can ever admit ends at once as a failure its owner reports.
use super::closing_tests::{Fixture, terminal};
use super::*;
use crate::completion_wake::Wake;
use sha2::{Digest, Sha256};
use sugarsubstitute_application::workspace_session::close_save::{SaveFailure, SaveVerdict};
use sugarsubstitute_execution::ExecutionOwner;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

/// The whole retained-memory budget of the limited executor, below every workspace job.
const LIMITED_BYTES: u64 = 1024 * 1024;

/// An executor whose whole budget no workspace job fits, kept alive for the test.
struct Limited {
    _owner: ExecutionOwner,
}

/// Move the workspace's later jobs onto an executor that can never admit them.
fn limit(fixture: &mut Fixture) -> TestResult<Limited> {
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::with_memory_capacity(
        fixture.host.clock.clone(),
        wake,
        std::num::NonZeroU64::new(LIMITED_BYTES).ok_or("Empty limited budget")?,
    )?;
    let client = owner.client();
    let work = fixture.host.workspace.as_mut().ok_or("Workspace")?;
    work.scope =
        Some(client.create_scope(ScopeSpec { id: ScopeId::new(9)?, deadline: None }, None)?);
    work.client = Some(client);
    Ok(Limited { _owner: owner })
}

/// Request the final capture and deliver it, returning its decode event.
fn final_capture(fixture: &mut Fixture) -> TestResult<DriverEvent> {
    let now = fixture.host.clock.now();
    let cutoff = now.checked_add(Duration::from_secs(10))?;
    let work = fixture.host.workspace.as_mut().ok_or("Workspace")?;
    work.input_directory = Some(fixture.directory.path().join("input-canvas"));
    let driver = work.driver.as_mut().ok_or("Driver")?;
    driver.begin_close(now, cutoff)?;
    driver.tick(now)?;
    let request = driver.next_message().ok_or("Final capture absent")?;
    let SessionOperation::Capture { checkpoint_id, revision, .. } = &request.operation else {
        return Err("Wrong final operation".into());
    };
    let mut checkpoint = fixture.checkpoint.clone();
    checkpoint.id = checkpoint_id.clone();
    checkpoint.revision = *revision;
    checkpoint.previous_id = None;
    let bytes = sugarsubstitute_contracts::desktop_session::encode_checkpoint(&checkpoint)?;
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
    message.operation = SessionOperation::Commit { transfer_id };
    driver.receive(message, now)?.ok_or_else(|| "Final decode absent".into())
}

#[test]
fn a_capture_decode_no_executor_can_admit_fails_the_final_save() -> TestResult {
    let mut fixture = Fixture::new(true)?;
    let event = final_capture(&mut fixture)?;
    let _limited = limit(&mut fixture)?;
    let now = fixture.host.clock.now();
    let work = fixture.host.workspace.as_mut().ok_or("Workspace")?;
    work.pending = Some(event);
    work.advance(now)?;
    assert!(work.pending.is_none() && work.active.is_none(), "nothing waits for admission");
    let verdict = work.driver.as_ref().ok_or("Driver")?.close_verdict(now);
    assert_eq!(verdict, Some(SaveVerdict::Failed(SaveFailure::Storage)));
    Ok(())
}

#[test]
fn a_publication_no_executor_can_admit_fails_the_final_save() -> TestResult {
    let mut fixture = Fixture::new(true)?;
    let profile = open_profile(&fixture)?;
    let event = final_capture(&mut fixture)?;
    let now = fixture.host.clock.now();
    let work = fixture.host.workspace.as_mut().ok_or("Workspace")?;
    let DriverEvent::DecodeCapture { request_id, ticket, transfer_id, transfer } = event else {
        return Err("Final decode absent".into());
    };
    let mut captured = transfer.finish(&transfer_id, now)?;
    let driver = work.driver.as_mut().ok_or("Driver")?;
    driver.decoded_capture(&request_id, &ticket, &mut captured, now)?;
    work.capture = Some((ticket, captured));
    work.profile = Some(Arc::new(Mutex::new(profile)));
    let context = work.context.clone().ok_or("Context")?;
    let inputs = crate::input_documents::workspace::WorkspaceInputs {
        archive: sugarsubstitute_canvas::InputDocumentArchive::new(Vec::new(), b"")?,
        generation: 0,
        surfaces: Vec::new(),
    };
    work.offer_inputs(inputs, context)?;
    let _limited = limit(&mut fixture)?;
    let work = fixture.host.workspace.as_mut().ok_or("Workspace")?;
    work.submit_publication(now)?;
    assert!(work.publish_input.is_none() && work.active.is_none(), "nothing waits for admission");
    let verdict = work.driver.as_ref().ok_or("Driver")?.close_verdict(now);
    assert_eq!(verdict, Some(SaveVerdict::Failed(SaveFailure::Storage)));
    Ok(())
}

#[test]
fn a_restore_encoding_no_executor_can_admit_ends_the_restore() -> TestResult {
    let mut fixture = Fixture::new(true)?;
    let vectors: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../third_party/sugarcubes-session/session_vectors.json"
    ))?;
    let checkpoint = sugarsubstitute_contracts::desktop_session::decode_checkpoint(
        &serde_json::to_vec(&vectors["checkpoints"][0]["value"])?,
    )?;
    let mut fence: SessionFence = serde_json::from_value(vectors["currentFence"].clone())?;
    fence.revision = 1;
    let store = WorkspaceStore::new(
        fixture.directory.path().join("restore-checkpoints"),
        fixture.directory.path().join("restore-assets"),
        checkpoint.scope.clone(),
    )?;
    let work = fixture.host.workspace.as_ref().ok_or("Workspace")?;
    let client = work.client.as_ref().ok_or("Client")?;
    let scope = work.scope.as_ref().ok_or("Scope")?;
    let spec = |id| JobSpec {
        id,
        operation: Operation::Infrastructure,
        class: JobClass::InfrastructureOrdinary,
        deadline: Some(crate::job_deadline::OPEN_ENDED),
        reservation_bytes: WorkspaceStore::reservation_bytes(),
        replacement: None,
    };
    // A previous run's checkpoint with tabs to restore, as the page published it.
    let checkpoints = fixture.directory.path().join("restore-checkpoints");
    std::fs::create_dir_all(&checkpoints)?;
    std::fs::write(
        checkpoints.join(workspace_session::CURRENT_CHECKPOINT),
        sugarsubstitute_contracts::desktop_session::encode_checkpoint(&checkpoint)?,
    )?;
    let loaded = store.submit_load(client, scope, spec(work.jobs.allocate()?))?;
    let JobOutcome::Success(restored) = terminal(client, &fixture.wake, loaded)? else {
        return Err("The restore checkpoint did not load".into());
    };
    let mut driver = WorkspaceDriver::new(
        checkpoint.scope.clone(),
        vectors["origin"].as_str().ok_or("Missing origin")?.into(),
        fence.webview.clone(),
        restored,
        MemoryLease::new(WorkspaceDriver::reservation_bytes(), Box::new(NoRelease)),
    )?;
    let ready = SessionMessage {
        protocol: "sugarcubes.desktop-session".into(),
        version: 1,
        request_id: "00000000-0000-4000-8000-000000000099".into(),
        scope: checkpoint.scope.clone(),
        origin: vectors["origin"].as_str().ok_or("Missing origin")?.into(),
        operation: SessionOperation::Ready {
            fence,
            capabilities: SessionCapabilities {
                capture: true,
                materialize: true,
                chunk_transfer: true,
                artifact_resolution: true,
            },
            committed: true,
            workflow_persistence: true,
        },
    };
    let now = fixture.host.clock.now();
    let Some(event @ DriverEvent::EncodeRestore { .. }) = driver.receive(ready, now)? else {
        return Err("Restore encoding absent".into());
    };
    let DriverEvent::EncodeRestore { request_id, .. } = &event else {
        return Err("Restore encoding absent".into());
    };
    let request_id = request_id.clone();
    let _limited = limit(&mut fixture)?;
    let work = fixture.host.workspace.as_mut().ok_or("Workspace")?;
    work.driver = Some(driver);
    work.pending = Some(event);
    work.advance(now)?;
    assert!(work.pending.is_none() && work.active.is_none(), "nothing waits for admission");
    assert!(!work.driver.as_ref().ok_or("Driver")?.owns_restore(&request_id), "the restore ended");
    Ok(())
}

struct NoRelease;
impl MemoryRelease for NoRelease {
    fn release(&self, _: u64) {}
}

/// Open the fixture's profile on its full-capacity executor.
fn open_profile(fixture: &Fixture) -> TestResult<OpenedProfile> {
    let work = fixture.host.workspace.as_ref().ok_or("Workspace")?;
    let client = work.client.as_ref().ok_or("Client")?;
    let context = work.context.as_ref().ok_or("Context")?;
    let store = ProfileStore::new(
        fixture.directory.path(),
        sugarsubstitute_contracts::desktop_session::InstallationId::new(
            context.installation.as_str().to_owned(),
        )?,
        sugarsubstitute_contracts::desktop_session::ProfileId::new(
            context.profile.as_str().to_owned(),
        )?,
    )?;
    let job = store.submit_open(
        client,
        work.scope.as_ref().ok_or("Scope")?,
        JobSpec {
            id: work.jobs.allocate()?,
            operation: Operation::Infrastructure,
            class: JobClass::InfrastructureOrdinary,
            deadline: Some(crate::job_deadline::OPEN_ENDED),
            reservation_bytes: ProfileStore::reservation_bytes(),
            replacement: None,
        },
        context.backend,
    )?;
    match terminal(client, &fixture.wake, job)? {
        JobOutcome::Success(profile) => Ok(profile),
        other => Err(format!("Profile open stopped: {:?}", other.kind()).into()),
    }
}
