// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify one original request, immutable cutoff and retained rejected evidence without windows.

use super::*;
use crate::production::qualification::QualificationIntent;
use std::{ffi::OsString, sync::Arc};
use sugarsubstitute_contracts::{
    configuration::*,
    lifecycle::*,
    startup::{LaunchId, ReadinessReceipt, StartupEvent, StartupObservation},
};
use sugarsubstitute_execution::ExecutionOwner;
#[path = "deadline_tests.rs"]
mod deadline;

type TestResult = Result<(), Box<dyn Error>>;
struct Fixed;
impl Clock for Fixed {
    fn now(&self) -> Moment {
        Moment::from_nanos(17)
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
struct Wake;
impl CompletionWake for Wake {
    fn wake(&self) -> Result<(), ExecutionError> {
        Ok(())
    }
}
fn main() -> Result<MainIdentity, Box<dyn Error>> {
    Ok(MainIdentity {
        run: RunKey {
            owner: CoordinatorKey {
                namespace: NamespaceId::new([1; 32]),
                epoch: CoordinatorEpoch::new([2; 16])?,
            },
            instance: InstanceId::new(1)?,
            generation: RunGeneration::new(1)?,
        },
        launch: LaunchId::new(1)?,
        navigation: Generation::new(1)?,
    })
}
fn preview() -> Result<Preview, Box<dyn Error>> {
    preview_until(Moment::from_nanos(100))
}
fn preview_until(until: Moment) -> Result<Preview, Box<dyn Error>> {
    let mut arguments = [
        "--internal-k1-native-preview",
        "--preview-until-qpc",
        "100",
        "--preview-qpc-frequency",
        "10",
        "--preview-destination",
        "D:\\fixture\\actual.png",
    ]
    .into_iter()
    .map(OsString::from);
    let intent = QualificationIntent::child(&mut arguments)?
        .take_preview()
        .ok_or("Actual lexical preview intent absent")?;
    Ok(Preview::fixture_new(intent, main()?.run, until)?)
}
fn failed(identity: MainIdentity) -> Result<DesktopStartupObservation, Box<dyn Error>> {
    Ok(DesktopStartupObservation::MainPreviewFailed {
        identity,
        cause: crate::desktop_error_fixture::native_failure()?,
    })
}

/// Fake external acknowledgements drive the real domain session through its actual contract.
fn session(running: bool) -> Result<StartupSession, Box<dyn Error>> {
    let identity = main()?;
    let origin = LoopbackOrigin::parse("http://127.0.0.1:8188/", 8188)?;
    let configuration = StartupConfiguration::new(
        ConnectionConfiguration::Attach(origin),
        ProfileId::new("preview-fixture".into())?,
        Presentation::new("en", Theme::Dark, true)?,
    );
    let mut session = StartupSession::new(identity.launch, identity.navigation, configuration);
    for observation in
        [StartupObservation::SplashPresented, StartupObservation::ConnectionReady(origin)]
    {
        assert!(session.observe(StartupEvent::new(identity.launch, observation))?.is_some());
    }
    assert!(
        session
            .observe(StartupEvent::new(
                identity.launch,
                StartupObservation::FrontendNavigationStarted(identity.navigation)
            ))?
            .is_none()
    );
    if running {
        let receipt = ReadinessReceipt::parse(
            br#"{"schema_version":1,"launch_id":1,
            "navigation_generation":1,"outcome":{"status":"ready","evidence":{
            "frontend_version":"1.47.11","sugarcubes_version":"0.15.0",
            "app_graph_ready":true,"canvas_connected":true,"loading_finished":true,
            "sugarcubes_registered":true,"sugarcubes_ready":true}}}"#,
        )?;
        for observation in
            [StartupObservation::FrontendReceipt(receipt), StartupObservation::SplashRetired]
        {
            assert!(session.observe(StartupEvent::new(identity.launch, observation))?.is_some());
        }
        assert!(
            session
                .observe(StartupEvent::new(identity.launch, StartupObservation::MainRevealed))?
                .is_none()
        );
    }
    Ok(session)
}

#[test]
fn running_preview_keeps_original_campaign_after_completed_frontend_setup() -> TestResult {
    let runtime = ExecutionOwner::new(Arc::new(Fixed), Arc::new(Wake))?;
    let client = runtime.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let session = session(true)?;
    let mut owner = preview_until(Moment::from_nanos(7_200_000_000_000))?;
    let frontend =
        sugarsubstitute_application::startup::FrontendBudget::new(Moment::from_nanos(0))?;
    let identity = main()?;
    let command = owner
        .after_main_revealed(
            identity,
            &session,
            &scope,
            &scope,
            Moment::from_nanos(frontend.until().as_nanos() + 1),
        )?
        .ok_or("Native preview command absent")?;
    assert!(matches!(command, DesktopStartupCommand::CaptureMainPreview {
        identity: current, source_deadline, downloads: None,
    } if current == identity && source_deadline == CounterDeadline::new(100, 10)?));
    assert!(matches!(
        owner.after_main_revealed(
            identity,
            &session,
            &scope,
            &scope,
            Moment::from_nanos(31_000_000_000)
        ),
        Err(PreviewError::State)
    ));
    assert_eq!(owner.until, Moment::from_nanos(7_200_000_000_000));
    let mut expired = preview()?;
    assert!(matches!(
        expired.after_main_revealed(identity, &session, &scope, &scope, Moment::from_nanos(100)),
        Err(PreviewError::Deadline)
    ));
    Ok(())
}

#[test]
fn foreign_and_duplicate_preview_failures_preserve_observation_custody() -> TestResult {
    let runtime = ExecutionOwner::new(Arc::new(Fixed), Arc::new(Wake))?;
    let client = runtime.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let jobs = JobSequence::new();
    let mut owner = preview()?;
    let identity = main()?;
    assert!(
        owner
            .after_main_revealed(identity, &session(true)?, &scope, &scope, Moment::from_nanos(17))?
            .is_some()
    );
    let mut foreign = identity;
    foreign.launch = LaunchId::new(2)?;
    let mut observation = Some(failed(foreign)?);
    assert!(matches!(
        owner.accept(&mut observation, &client, &scope, &jobs),
        Err(PreviewError::Identity)
    ));
    assert!(observation.is_some());
    assert!(!owner.received);
    let mut current = Some(failed(identity)?);
    assert!(owner.accept(&mut current, &client, &scope, &jobs)?);
    assert!(current.is_none());
    assert!(matches!(owner.failure, Some(PreviewFailure::Native(_))));
    let mut duplicate = Some(failed(identity)?);
    assert!(owner.accept(&mut duplicate, &client, &scope, &jobs).is_err());
    assert!(duplicate.is_some());
    Ok(())
}

#[test]
fn rejected_slot_is_bounded_and_close_retains_until_explicit_poll_disposal() -> TestResult {
    let mut owner = preview()?;
    let mut rejected = Some(failed(main()?)?);
    assert!(owner.reject_retained(&mut rejected)?);
    assert!(rejected.is_none());
    assert!(owner.pending());
    let mut excess = Some(failed(main()?)?);
    assert!(matches!(owner.reject_retained(&mut excess), Err(PreviewError::State)));
    assert!(excess.is_some());
    owner.close();
    assert!(owner.pending());
    assert!(!owner.settled());
    owner.poll(Moment::from_nanos(18))?;
    assert!(owner.settled());
    Ok(())
}

#[test]
fn loading_stale_closed_and_cancelled_admission_do_not_issue_preview_command() -> TestResult {
    let runtime = ExecutionOwner::new(Arc::new(Fixed), Arc::new(Wake))?;
    let client = runtime.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let mut owner = preview()?;
    let identity = main()?;
    assert!(matches!(
        owner.after_main_revealed(
            identity,
            &session(false)?,
            &scope,
            &scope,
            Moment::from_nanos(17)
        ),
        Err(PreviewError::State)
    ));
    assert!(!owner.requested);
    let mut foreign = identity;
    foreign.navigation = Generation::new(2)?;
    assert!(matches!(
        owner.after_main_revealed(foreign, &session(true)?, &scope, &scope, Moment::from_nanos(17)),
        Err(PreviewError::Identity)
    ));
    assert!(!owner.requested);
    let mut closed = preview()?;
    closed.close();
    assert!(matches!(
        closed.after_main_revealed(
            identity,
            &session(true)?,
            &scope,
            &scope,
            Moment::from_nanos(17)
        ),
        Err(PreviewError::State)
    ));
    scope.cancel();
    assert!(matches!(
        owner.after_main_revealed(
            identity,
            &session(true)?,
            &scope,
            &scope,
            Moment::from_nanos(17)
        ),
        Err(PreviewError::State)
    ));
    assert!(!owner.requested);
    Ok(())
}

#[test]
fn cancelled_native_transport_scope_rejects_even_when_ordinary_scope_remains_live() -> TestResult {
    let runtime = ExecutionOwner::new(Arc::new(Fixed), Arc::new(Wake))?;
    let client = runtime.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let native =
        client.create_native_scope(ScopeSpec { id: ScopeId::new(3)?, deadline: None }, None)?;
    native.cancel();
    assert!(!scope.snapshot().cancellation_requested);
    let mut owner = preview()?;
    assert!(matches!(
        owner.after_main_revealed(
            main()?,
            &session(true)?,
            &scope,
            &native,
            Moment::from_nanos(17)
        ),
        Err(PreviewError::State)
    ));
    assert!(!owner.requested);
    Ok(())
}

#[test]
fn ordinary_native_observation_keeps_its_original_route() -> TestResult {
    let runtime = ExecutionOwner::new(Arc::new(Fixed), Arc::new(Wake))?;
    let client = runtime.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let mut owner = preview()?;
    let mut ordinary = Some(DesktopStartupObservation::MainAbsent(main()?));
    assert!(!owner.accept(&mut ordinary, &client, &scope, &JobSequence::new())?);
    assert!(matches!(ordinary, Some(DesktopStartupObservation::MainAbsent(_))));
    assert!(!owner.reject_retained(&mut ordinary)?);
    assert!(!owner.pending());
    Ok(())
}

#[test]
fn digest_formatter_writes_fixed_public_hex_without_dynamic_format_storage() -> TestResult {
    use std::fmt::Write;
    struct Fixed {
        bytes: [u8; 64],
        length: usize,
    }
    impl fmt::Write for Fixed {
        fn write_str(&mut self, text: &str) -> fmt::Result {
            let end = self.length.checked_add(text.len()).ok_or(fmt::Error)?;
            let destination = self.bytes.get_mut(self.length..end).ok_or(fmt::Error)?;
            destination.copy_from_slice(text.as_bytes());
            self.length = end;
            Ok(())
        }
    }
    let mut output = Fixed { bytes: [0; 64], length: 0 };
    write!(output, "{}", worker::Digest(&[0xab; 32]))?;
    assert_eq!(output.length, 64);
    assert!(output.bytes.chunks_exact(2).all(|pair| pair == b"ab"));
    Ok(())
}

#[test]
fn cumulative_persister_layout_stays_within_original_sixty_four_kib() {
    println!("preview_controller_storage_bytes={}", Preview::storage_bytes());
    assert!(worker::layout_fits(4096));
    assert!(!worker::layout_fits(4097));
    assert!(!worker::layout_fits(usize::MAX));
}

#[test]
fn missing_genuine_source_preserves_original_constructor_input() -> TestResult {
    let mut intent = preview()?.intent;
    let original = intent.as_ref().ok_or("Missing fixture intent")?;
    let deadline = original.deadline();
    let capacity = original.allocated_bytes();
    let mut source = None;
    assert!(matches!(
        Preview::new(&mut intent, main()?.run, &mut source),
        Err(PreviewError::State)
    ));
    let retained = intent.as_ref().ok_or("Rejected constructor consumed intent")?;
    assert_eq!(retained.deadline(), deadline);
    assert_eq!(retained.allocated_bytes(), capacity);
    assert_eq!(retained.destination(), std::path::Path::new("D:\\fixture\\actual.png"));
    assert!(source.is_none());
    Ok(())
}

#[test]
fn admitted_exact_retirement_cancels_requested_main_but_foreign_and_early_retry_preserve_it()
-> TestResult {
    let runtime = ExecutionOwner::new(Arc::new(Fixed), Arc::new(Wake))?;
    let client = runtime.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let current = main()?;
    let mut owner = preview()?;
    owner.retire_main(current);
    assert!(!owner.closing);
    assert!(owner.intent.is_some());
    assert_eq!(owner.until, Moment::from_nanos(100));
    assert!(
        owner
            .after_main_revealed(current, &session(true)?, &scope, &scope, Moment::from_nanos(17))?
            .is_some()
    );
    let foreign = MainIdentity { navigation: Generation::new(2)?, ..current };
    owner.retire_main(foreign);
    assert!(!owner.closing);
    assert!(owner.intent.is_some());
    owner.retire_main(current);
    assert!(owner.closing);
    assert_eq!(owner.until, Moment::from_nanos(100));
    Ok(())
}

#[test]
fn mismatched_notice_retains_real_panicked_worker_terminal_and_original_credit() -> TestResult {
    let wake = Arc::new(crate::completion_wake::Wake::default());
    let runtime = ExecutionOwner::new(Arc::new(Fixed), wake.clone())?;
    let client = runtime.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let spec = JobSpec {
        id: JobId::new(1)?,
        operation: Operation::Infrastructure,
        class: JobClass::InfrastructureOrdinary,
        deadline: Some(Moment::from_nanos(100)),
        reservation_bytes: worker::JOB_BYTES,
        replacement: None,
    };
    let job = client.submit_blocking(&scope, spec, |_| -> Result<Persisted, PersistFailure> {
        // Fake only an external worker failure; central terminal/ownership remain real.
        std::panic::resume_unwind(Box::new("bounded external preview worker failure"))
    })?;
    let mut owner = preview()?;
    owner.job = Some(job);
    let batch = wake.until(|| {
        let batch = match client.drain_completions(std::num::NonZeroUsize::MAX) {
            Ok(batch) => batch,
            Err(cause) => return Some(Err::<_, Box<dyn Error>>(cause.into())),
        };
        batch
            .notifications
            .iter()
            .any(|notice| notice.scope_id == scope.id() && notice.job_id == spec.id)
            .then_some(Ok(batch))
    })?;
    owner.capture(&batch);
    assert_eq!(owner.notice.as_ref().map(|notice| notice.kind), Some(TerminalKind::Panicked));
    owner.notice.as_mut().ok_or("Real notice missing")?.kind = TerminalKind::Success;
    owner.poll(Moment::from_nanos(17))?;
    assert!(owner.terminal_rejected);
    assert!(!owner.reported);
    assert!(owner.pending());
    let terminal = owner.terminal.take().ok_or("Real rejected completion disappeared")?;
    assert_eq!(terminal.scope_id, scope.id());
    assert_eq!(terminal.spec, spec);
    assert_eq!(terminal.memory.bytes(), worker::JOB_BYTES);
    assert!(terminal.native_handles.is_none());
    assert!(matches!(&terminal.outcome, JobOutcome::Panicked(_)));
    // Explicit test-owner disposal follows real observed stopped work and no native inputs.
    drop(terminal);
    assert!(!owner.pending());
    Ok(())
}
