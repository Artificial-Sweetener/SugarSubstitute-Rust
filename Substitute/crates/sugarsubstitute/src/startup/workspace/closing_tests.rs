// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep the native peer alive for a final checkpoint while ordinary frame polling drives close.
use super::*;
use crate::completion_wake::Wake;
use crate::startup::platform_tests::{Platform, Prepared};
use sha2::{Digest, Sha256};
use std::num::NonZeroUsize;
use sugarsubstitute_application::startup::StartupSession;
use sugarsubstitute_contracts::{
    configuration::*,
    lifecycle::ShutdownBudget,
    startup::{ReadinessReceipt, StartupEvent, StartupObservation},
};
use sugarsubstitute_desktop::{MainPlacementObservation, StartupWindowContent, WindowContent};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;
struct Release;
impl MemoryRelease for Release {
    fn release(&self, _: u64) {}
}
pub(super) struct Fixture {
    pub(super) host: StartupHost<Platform>,
    checkpoint: SessionCheckpoint,
    _directory: tempfile::TempDir,
    pub(super) wake: Arc<Wake>,
    store: WorkspaceStore,
}
/// Deliver completions until the job has its typed terminal outcome, then take it.
fn terminal<T, E>(
    client: &ExecutionClient,
    wake: &Wake,
    mut job: OwnedJob<T, E>,
) -> TestResult<JobOutcome<T, E>> {
    Ok(wake.completion(client, &mut job)?.outcome)
}
impl Fixture {
    pub(super) fn new(persistence: bool) -> TestResult<Self> {
        let vectors: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../../third_party/sugarcubes-session/session_vectors.json"
        ))?;
        let mut checkpoint = sugarsubstitute_contracts::desktop_session::decode_checkpoint(
            &serde_json::to_vec(&vectors["checkpoints"][0]["value"])?,
        )?;
        checkpoint.entries.clear();
        checkpoint.active_entry = None;
        checkpoint.assets.clear();
        let mut fence: SessionFence = serde_json::from_value(vectors["currentFence"].clone())?;
        fence.revision = 1;
        fence.bindings.clear();
        let directory = tempfile::tempdir()?;
        let mut host = crate::startup::platform_tests::host()?;
        host.child.start_allowed = true;
        host.execution.reserve_workspace_capacity(CAPACITY)?;
        host.workspace = Some(WorkspaceWork::new(
            directory.path().to_path_buf(),
            ScopeSpec { id: ScopeId::new(3)?, deadline: None },
            host.jobs.clone(),
        ));
        let wake = Arc::new(Wake::default());
        host.start(wake.clone())?;
        let origin = LoopbackOrigin::parse(vectors["origin"].as_str().ok_or("Origin")?, 8188)?;
        let mut session = StartupSession::new(
            host.launch_id(),
            Generation::new(1)?,
            StartupConfiguration::new(
                ConnectionConfiguration::Attach(origin),
                ProfileId::new("closing-test".into())?,
                Presentation::new("en", Theme::Dark, true)?,
            ),
        );
        for observation in [
            StartupObservation::SplashPresented,
            StartupObservation::ConnectionReady(origin),
            StartupObservation::FrontendNavigationStarted(Generation::new(1)?),
        ] {
            session.observe(StartupEvent::new(host.launch_id(), observation))?;
        }
        let receipt = ReadinessReceipt::parse(br#"{"schema_version":1,"launch_id":1,"navigation_generation":1,"outcome":{"status":"ready","evidence":{"frontend_version":"1.0.0","sugarcubes_version":"1.0.0","app_graph_ready":true,"canvas_connected":true,"loading_finished":true,"sugarcubes_registered":true,"sugarcubes_ready":true}}}"#)?;
        session.observe(StartupEvent::new(
            host.launch_id(),
            StartupObservation::FrontendReceipt(receipt),
        ))?;
        session.observe(StartupEvent::new(host.launch_id(), StartupObservation::SplashRetired))?;
        session.observe(StartupEvent::new(host.launch_id(), StartupObservation::MainRevealed))?;
        assert_eq!(session.phase(), sugarsubstitute_application::startup::StartupPhase::Running);
        host.startup = Some(Prepared { session, released: false });
        let context = WorkspaceContext {
            identity: host.main_identity().ok_or("Main identity")?,
            profile: ProfileId::new("closing-test".into())?,
            installation: sugarsubstitute_contracts::installation::InstallationId::new(
                "01".repeat(16),
            )?,
            backend: sugarsubstitute_contracts::lifecycle::BackendBindingDigest::from_bytes(
                [1; 32],
            ),
            origin,
            peer: sugarsubstitute_supervision::lifecycle_protocol::BackendPeer {
                process_id: std::num::NonZeroU32::MIN,
                creation_time_100ns: std::num::NonZeroU64::MIN,
            },
        };
        let workspace = host.workspace.as_mut().ok_or("Workspace")?;
        let client = workspace.client.as_ref().ok_or("Client")?;
        let store = WorkspaceStore::new(
            directory.path().join("checkpoints"),
            directory.path().join("assets"),
            checkpoint.scope.clone(),
        )?;
        let load = store.submit_load(
            client,
            workspace.scope.as_ref().ok_or("Scope")?,
            JobSpec {
                id: workspace.jobs.allocate()?,
                operation: Operation::Infrastructure,
                class: JobClass::InfrastructureOrdinary,
                deadline: Some(host.clock.now().checked_add(Duration::from_secs(30))?),
                reservation_bytes: WorkspaceStore::reservation_bytes(),
                replacement: None,
            },
        )?;
        let restored = match terminal(client, &wake, load)? {
            JobOutcome::Success(restored) => restored,
            other => return Err(format!("Load stopped: {:?}", other.kind()).into()),
        };
        let bootstrap =
            SessionBootstrap::new(checkpoint.scope.clone(), origin, fence.webview.clone())?;
        let mut driver = WorkspaceDriver::new(
            checkpoint.scope.clone(),
            bootstrap.origin_text(),
            fence.webview.clone(),
            restored,
            MemoryLease::new(DRIVER_BYTES, Box::new(Release)),
        )?;
        let ready = SessionMessage {
            protocol: "sugarcubes.desktop-session".into(),
            version: 1,
            request_id: fence.webview.clone(),
            scope: checkpoint.scope.clone(),
            origin: bootstrap.origin_text(),
            operation: SessionOperation::Ready {
                fence,
                capabilities: SessionCapabilities {
                    capture: true,
                    materialize: true,
                    chunk_transfer: true,
                    artifact_resolution: true,
                },
                committed: true,
                workflow_persistence: persistence,
            },
        };
        assert!(driver.receive(ready, host.clock.now())?.is_none());
        workspace.driver = Some(driver);
        workspace.context = Some(context);
        workspace.bootstrap = Some(bootstrap);
        workspace.delivery = Some(MemoryLease::new(DELIVERY_BYTES, Box::new(Release)));
        Ok(Self { host, checkpoint, _directory: directory, wake, store })
    }
    fn poll(&mut self) -> TestResult {
        assert!(self.host.poll_frame(NonZeroUsize::MIN.saturating_add(31))?.is_none());
        Ok(())
    }
    fn pending(&mut self) -> TestResult {
        self.poll()?;
        assert!(self.host.child.shutdown_requests.is_empty());
        assert_eq!(self.host.child.shutdown_starts, 0);
        assert!(!matches!(self.host.take_command()?, Some(DesktopStartupCommand::Close)));
        Ok(())
    }
    fn sample(&mut self) -> TestResult {
        let Some(DesktopStartupCommand::CaptureMainPlacement { identity, capture_id }) =
            self.host.take_command()?
        else {
            return Err("Final placement request absent".into());
        };
        self.host.observe(DesktopStartupObservation::MainPlacementObserved(
            MainPlacementObservation {
                identity,
                sequence: std::num::NonZeroU64::MIN,
                capture_id: Some(capture_id),
                placement: self.checkpoint.placement.clone().ok_or("Placement vector absent")?,
            },
        ))?;
        Ok(())
    }
    fn save(&self, checkpoint: SessionCheckpoint) -> TestResult<workspace_session::SaveReceipt> {
        let workspace = self.host.workspace.as_ref().ok_or("Workspace")?;
        let client = workspace.client.as_ref().ok_or("Client")?;
        let job = self.store.submit_save(
            client,
            workspace.scope.as_ref().ok_or("Scope")?,
            JobSpec {
                id: workspace.jobs.allocate()?,
                operation: Operation::Infrastructure,
                class: JobClass::InfrastructureOrdinary,
                deadline: Some(self.host.shutdown.budget()?.graceful_until),
                reservation_bytes: WorkspaceStore::reservation_bytes(),
                replacement: None,
            },
            checkpoint,
        )?;
        match terminal(client, &self.wake, job)? {
            JobOutcome::Success(receipt) => Ok(receipt),
            other => Err(format!("Save stopped: {:?}", other.kind()).into()),
        }
    }
    fn finish(mut self, budget: ShutdownBudget, starts: usize) -> TestResult {
        self.poll()?;
        assert_eq!(self.host.child.shutdown_requests, vec![budget]);
        assert_eq!(self.host.child.shutdown_starts, starts);
        assert!(matches!(self.host.take_command()?, Some(DesktopStartupCommand::Close)));
        assert_eq!(self.host.shutdown.budget()?, budget);
        assert!(self.host.take_command()?.is_none());
        crate::startup::platform_tests::consume::finish(&mut self.host, &self.wake)?;
        Ok(())
    }
}

#[test]
fn poll_frame_releases_opted_out_close_at_the_earlier_observed_capture_cutoff() -> TestResult {
    let mut fixture = Fixture::new(false)?;
    fixture.host.observe(DesktopStartupObservation::CloseRequested)?;
    let original = fixture.host.shutdown.budget()?;
    fixture.sample()?;
    fixture.pending()?;
    let cutoff = ShutdownBudget {
        graceful_until: Moment::from_nanos(0),
        ..ShutdownBudget::new(Moment::from_nanos(0))?
    };
    cutoff.validate()?;
    assert!(cutoff.settle_until < original.settle_until);
    fixture.host.child.budget = Some(cutoff);
    fixture.finish(cutoff, 0)
}

#[test]
fn poll_frame_keeps_native_peer_until_exact_final_durable_delivery() -> TestResult {
    let mut fixture = Fixture::new(true)?;
    fixture.host.observe(DesktopStartupObservation::CloseRequested)?;
    let budget = fixture.host.shutdown.budget()?;
    fixture.sample()?;
    fixture.poll()?;
    assert!(fixture.host.child.shutdown_requests.is_empty());
    assert_eq!(fixture.host.child.shutdown_starts, 0);
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
    let receipt = fixture.save(captured.checkpoint.clone())?;
    fixture
        .host
        .workspace
        .as_mut()
        .and_then(|work| work.driver.as_mut())
        .ok_or("Driver")?
        .published(&ticket, captured, &receipt, fixture.host.clock.now())?;
    fixture.poll()?;
    assert!(fixture.host.child.shutdown_requests.is_empty());
    assert_eq!(fixture.host.child.shutdown_starts, 0);
    let Some(DesktopStartupCommand::SendSession(delivery)) = fixture.host.take_command()? else {
        return Err("Durable acknowledgement absent".into());
    };
    assert!(
        matches!(delivery.message().operation, SessionOperation::Acknowledged { ref phase, .. } if phase == "durable")
    );
    fixture.pending()?;
    let now = fixture.host.clock.now();
    let driver =
        fixture.host.workspace.as_mut().and_then(|work| work.driver.as_mut()).ok_or("Driver")?;
    let mut foreign = delivery.message().clone();
    foreign.request_id = "00000000-0000-4000-8000-000000000098".into();
    assert!(driver.receive(foreign, now).is_err());
    assert!(!driver.close_ready(now));
    assert!(driver.receive(delivery.message().clone(), now)?.is_none());
    fixture.finish(budget, 1)
}
