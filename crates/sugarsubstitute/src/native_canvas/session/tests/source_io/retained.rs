// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise generated-image custody, replacement and shutdown through real worker boundaries.

use super::super::{refinement::Wake, *};
use std::sync::{Condvar, Mutex};
use sugarsubstitute_image_ops::{Pixel, RasterSource};
use sugarsubstitute_supervision::cache::{Namespace, RuntimeCache, storage_key};

#[cfg(feature = "native-composition")]
mod acquisition;
#[cfg(feature = "native-composition")]
mod camera;
#[cfg(all(windows, feature = "native-composition"))]
mod composition;
mod frames;
#[cfg(feature = "native-composition")]
mod transactions;

const FIXTURE_TIME: u64 = 4_000_000_000;

/// Hold actual worker disposal while the application observes other terminal payloads.
#[derive(Default)]
struct DisposalGate {
    state: Mutex<(bool, bool)>,
    changed: Condvar,
}

impl DisposalGate {
    /// Publish worker entry and retain its input until the test explicitly releases custody.
    fn hold(&self) {
        let mut state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        state.0 = true;
        self.changed.notify_all();
        while !state.1 {
            state = self.changed.wait(state).unwrap_or_else(std::sync::PoisonError::into_inner);
        }
    }

    /// Bound fixture failure without using elapsed time to infer worker entry.
    fn wait_until_entered(&self) -> Result<(), Box<dyn Error>> {
        let state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let (state, timeout) = self
            .changed
            .wait_timeout_while(state, Duration::from_secs(10), |state| !state.0)
            .map_err(|cause| std::io::Error::other(cause.to_string()))?;
        if timeout.timed_out() && !state.0 {
            return Err("Source disposal worker did not enter".into());
        }
        Ok(())
    }

    /// Permit the worker to dispose of its original payload; repeated release is harmless.
    fn release(&self) {
        let mut state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        state.1 = true;
        self.changed.notify_all();
    }
}

/// Release a held worker even when setup or assertions fail.
struct ReleaseDisposal(Arc<DisposalGate>);

impl Drop for ReleaseDisposal {
    fn drop(&mut self) {
        self.0.release();
    }
}

struct Fixture {
    session: Session,
    clock: Arc<TestClock>,
    wake: Arc<Wake>,
    owner: ExecutionOwner,
    client: ExecutionClient,
    _directory: tempfile::TempDir,
}

impl Fixture {
    /// Start the same empty session and shared executor used by the embedded host.
    fn new() -> Result<Self, Box<dyn Error>> {
        Self::start(false)
    }

    /// Attach an external CPU surface before the same startup begins when requested.
    fn start(attached: bool) -> Result<Self, Box<dyn Error>> {
        let (mut session, clock, directory) = session()?;
        session.config.initial = InitialSource::AwaitHost;
        if attached {
            session.attach_cpu_surface()?;
        }
        session.input(Input::Drawable(false))?;
        let wake = Arc::new(Wake::default());
        let owner = ExecutionOwner::new(clock.clone(), wake.clone())?;
        let client = owner.client();
        session.start_runtime(Runtime::shared(
            client.clone(),
            ScopeSpec { id: ScopeId::new(1)?, deadline: None },
        )?)?;
        while session.resources.is_none() {
            let observed = wake.revision();
            client.drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
            session.poll(Duration::ZERO)?;
            if session.resources.is_none() {
                wake.wait_after(observed)?;
            }
        }
        Ok(Self { session, clock, wake, owner, client, _directory: directory })
    }

    /// Publish a small encoded fixture and return its actual transferred cache pin.
    fn encoded(&mut self, color: [u8; 3]) -> Result<ImageSource, Box<dyn Error>> {
        let cache = self.session.resources.as_ref().ok_or("Canvas cache absent")?.cache.clone();
        let scope = &self.session.runtime.as_ref().ok_or("Canvas scope absent")?.scope;
        let spec = self.session.spec(
            Operation::Infrastructure,
            JobClass::InfrastructureOrdinary,
            8 * 1024 * 1024,
            Duration::from_secs(30),
            None,
        )?;
        let mut job = self
            .client
            .submit_blocking(scope, spec, move |context| encode_source(cache, color, &context))?;
        let result = self.wake.completion(&self.client, &mut job)?;
        match result.outcome {
            JobOutcome::Success(source) => Ok(source),
            JobOutcome::Failure(cause) => Err(Box::new(cause)),
            other => Err(format!("Encoded fixture stopped: {:?}", other.kind()).into()),
        }
    }

    /// Deliver actual completions until the current retained source reaches a terminal state.
    fn finish_loading(&mut self) -> Result<(), Box<dyn Error>> {
        loop {
            let observed = self.wake.revision();
            self.client.drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
            self.session.poll(Duration::ZERO)?;
            if self.session.failed.is_some() {
                return Err("Canvas source loading failed".into());
            }
            if self.session.pending_open.is_none()
                && self.session.loading.is_none()
                && self.session.document.scene.as_ref().is_some_and(|scene| scene.source.is_some())
            {
                return Ok(());
            }
            self.wake
                .wait_after(observed)
                .map_err(|cause| format!("Canvas source loading: {cause}"))?;
        }
    }

    /// Close and join the canvas scope, then the same shared executor, under one cutoff.
    fn finish(mut self) -> Result<tempfile::TempDir, Box<dyn Error>> {
        let cutoff = Moment::from_nanos(10_000_000_000);
        self.session.close_until(cutoff).map_err(|cause| format!("Canvas close: {cause}"))?;
        loop {
            let observed = self.wake.revision();
            self.client
                .drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))
                .map_err(|cause| format!("Canvas completion drain: {cause}"))?;
            if self.session.settled().map_err(|cause| format!("Canvas settlement: {cause}"))? {
                break;
            }
            self.wake
                .wait_after(observed)
                .map_err(|cause| format!("Canvas scope settlement: {cause}"))?;
        }
        self.session.finish().map_err(|cause| format!("Canvas finish: {cause}"))?;
        self.owner.request_shutdown(cutoff);
        let deadline = std::time::Instant::now()
            .checked_add(Duration::from_secs(10))
            .ok_or("Execution shutdown deadline overflow")?;
        loop {
            self.client
                .drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))
                .map_err(|cause| format!("Execution completion drain: {cause}"))?;
            if self
                .owner
                .poll_shutdown()
                .map_err(|cause| format!("Execution settlement: {cause}"))?
                .settled
            {
                break;
            }
            if std::time::Instant::now() >= deadline {
                return Err("Shared execution did not settle".into());
            }
            std::hint::spin_loop();
        }
        self.owner.finish_shutdown()?;
        Ok(self._directory)
    }
}

fn encode_source(
    cache: RuntimeCache,
    color: [u8; 3],
    context: &JobContext,
) -> Result<ImageSource, WorkError> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, 1, 1);
        encoder.set_color(png::ColorType::Rgb);
        let mut writer = encoder.write_header().map_err(std::io::Error::other)?;
        writer.write_image_data(&color).map_err(std::io::Error::other)?;
    }
    let mut entry = cache.fresh(
        Namespace::ImageSources,
        &storage_key(&color),
        bytes.len() as u64,
        FIXTURE_TIME,
        context,
    )?;
    entry.write("output.png", &bytes, context)?;
    entry.publish(context)?;
    let mut input = Some(entry.into_file_reader("output.png", context)?);
    Ok(ImageSource::encoded(&mut input, context.memory())?)
}

#[test]
fn newer_retained_original_waits_for_bounded_disposal_and_is_the_only_loaded_image()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    let first = fixture.encoded([255, 0, 0])?;
    let second = fixture.encoded([0, 255, 0])?;
    let third = fixture.encoded([0, 0, 255])?;
    let mut first = Some(first);
    let mut second = Some(second);
    let mut third = Some(third);
    assert!(fixture.session.offer_source(&mut first, CacheReuse::Compatible, None)?);
    assert!(fixture.session.offer_source(&mut second, CacheReuse::Compatible, None)?);
    let selection = fixture.session.selection;
    assert!(!fixture.session.offer_source(&mut third, CacheReuse::Compatible, None)?);
    assert!(third.is_some());
    assert_eq!(fixture.session.selection, selection);
    assert!(
        fixture.session.pending_retirement.as_ref().is_some_and(|retired| retired._open.is_some())
    );

    fixture.session.poll(Duration::ZERO)?;
    assert!(fixture.session.retirement.is_some());
    assert!(fixture.session.offer_source(&mut third, CacheReuse::Compatible, None)?);
    assert!(third.is_none());
    assert!(
        fixture.session.pending_retirement.as_ref().is_some_and(|retired| retired._open.is_some())
    );
    fixture.finish_loading()?;

    let mut source = fixture
        .session
        .document
        .scene
        .as_ref()
        .and_then(|scene| scene.source.clone())
        .ok_or("Current source absent")?;
    let scope = &fixture.session.runtime.as_ref().ok_or("Canvas scope absent")?.scope;
    let spec = fixture.session.spec(
        Operation::Infrastructure,
        JobClass::InfrastructureOrdinary,
        1024 * 1024,
        Duration::from_secs(30),
        None,
    )?;
    let mut job = fixture.client.submit_blocking(scope, spec, move |context| {
        let descriptor = source.descriptor();
        let mut pixel = [Pixel::TRANSPARENT];
        source.read(
            descriptor.asset(),
            descriptor.layout().extent().region(),
            &mut pixel,
            &context,
        )?;
        Ok::<_, sugarsubstitute_image_ops::ImageError>(pixel[0])
    })?;
    let result = fixture.wake.completion(&fixture.client, &mut job)?;
    assert!(
        matches!(result.outcome, JobOutcome::Success(pixel) if pixel == Pixel::linear(0.0, 0.0, 1.0, 1.0)?)
    );
    drop(result.memory);
    drop(job);
    let cache = fixture.session.resources.as_ref().ok_or("Canvas cache absent")?.cache.clone();
    let scope = &fixture.session.runtime.as_ref().ok_or("Canvas scope absent")?.scope;
    let spec = fixture.session.spec(
        Operation::Infrastructure,
        JobClass::InfrastructureOrdinary,
        8 * 1024 * 1024,
        Duration::from_secs(30),
        None,
    )?;
    let mut maintenance = fixture.client.submit_blocking(scope, spec, move |context| {
        let now = FIXTURE_TIME
            .checked_add(Namespace::ImageSources.retention())
            .ok_or_else(|| std::io::Error::other("Retention clock overflow"))?;
        let report = cache.maintain(now, &context)?;
        Ok::<_, std::io::Error>((report.removed_entries, report.pinned_entries))
    })?;
    let result = fixture.wake.completion(&fixture.client, &mut maintenance)?;
    match result.outcome {
        JobOutcome::Success(counts) => assert_eq!(counts, (2, 2)),
        JobOutcome::Failure(cause) => return Err(Box::new(cause)),
        other => return Err(format!("Cache maintenance stopped: {:?}", other.kind()).into()),
    }
    drop(result.memory);
    drop(maintenance);
    let _directory = fixture.finish()?;
    Ok(())
}

#[test]
fn shutdown_retains_a_pending_encoded_original_until_worker_disposal() -> Result<(), Box<dyn Error>>
{
    let mut fixture = Fixture::new()?;
    let source = fixture.encoded([12, 34, 56])?;
    let mut source = Some(source);
    assert!(fixture.session.offer_source(&mut source, CacheReuse::Compatible, None)?);
    fixture.session.close_until(Moment::from_nanos(10_000_000_000))?;
    assert!(fixture.session.pending_open.is_some());
    fixture.session.poll(Duration::ZERO)?;
    assert!(fixture.session.pending_open.is_some() || fixture.session.retirement.is_some());
    let _directory = fixture.finish()?;
    Ok(())
}

#[test]
fn completed_initialization_waits_for_existing_source_disposal_during_shutdown()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    let mut first = Some(fixture.encoded([1, 2, 3])?);
    let mut second = Some(fixture.encoded([4, 5, 6])?);
    assert!(fixture.session.offer_source(&mut first, CacheReuse::Compatible, None)?);
    assert!(fixture.session.offer_source(&mut second, CacheReuse::Compatible, None)?);
    assert!(fixture.session.pending_retirement.is_some());

    let disposal = Arc::new(DisposalGate::default());
    let release = ReleaseDisposal(disposal.clone());
    let worker = disposal.clone();
    let disposal_spec = fixture.session.spec(
        Operation::Infrastructure,
        JobClass::InfrastructureLifecycle,
        0,
        Duration::from_secs(30),
        None,
    )?;
    let source_retirement = {
        let scope = &fixture.session.runtime.as_ref().ok_or("Canvas scope absent")?.scope;
        fixture.client.submit_blocking_owned_input(
            scope,
            disposal_spec,
            &mut fixture.session.pending_retirement,
            move |retired, _| {
                worker.hold();
                drop(retired);
                Ok::<(), WorkError>(())
            },
        )?
    };
    fixture.session.retirement = Some(source_retirement);
    disposal.wait_until_entered()?;

    let anchor = fixture._directory.path().join("second-cache");
    let scope = &fixture.session.runtime.as_ref().ok_or("Canvas scope absent")?.scope;
    let spec = fixture.session.spec(
        Operation::Infrastructure,
        JobClass::InfrastructureLifecycle,
        16 * 1024 * 1024,
        Duration::from_secs(30),
        None,
    )?;
    let initialization = fixture.client.submit_blocking(scope, spec, move |context| {
        work::initialize(anchor, false, false, &context)
    })?;
    loop {
        let observed = fixture.wake.revision();
        if let JobPhase::Terminal(kind) = initialization.status().phase {
            assert_eq!(kind, TerminalKind::Success);
            break;
        }
        fixture.wake.wait_after(observed)?;
    }
    fixture.session.initializing = Some(initialization);
    fixture.session.close_until(Moment::from_nanos(10_000_000_000))?;
    fixture.session.poll(Duration::ZERO)?;
    assert!(fixture.session.initializing.is_none());
    assert!(fixture.session.retirement.is_some());
    assert!(
        fixture
            .session
            .pending_retirement
            .as_ref()
            .is_some_and(|retired| { retired._resources.is_some() && retired._scene.is_some() })
    );
    assert!(fixture.session.pending_open.is_some());
    assert!(fixture.session.resources.is_some());
    assert!(!fixture.session.shutdown_started);
    release.0.release();
    let _directory = fixture.finish()?;
    Ok(())
}

#[test]
fn deadline_failure_keeps_encoded_retirement_until_the_original_shutdown_cutoff()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    let mut original = Some(fixture.encoded([9, 8, 7])?);
    assert!(fixture.session.offer_source(&mut original, CacheReuse::Compatible, None)?);
    let pending = fixture.session.pending_open.take().ok_or("Original was not retained")?;
    fixture.session.clock = Arc::new(TestClock(AtomicU64::new(u64::MAX - 1)));
    assert!(
        fixture.session.retire(Retired { _open: Some(pending), ..Default::default() }).is_err()
    );
    assert!(fixture.session.pending_retirement.as_ref().is_some_and(|value| value._open.is_some()));
    assert_eq!(fixture.session.retirement_deadline, None);

    // Restore the executor's clock after injecting the deadline-arithmetic failure.
    fixture.session.clock = fixture.clock.clone();
    fixture.session.close_until(Moment::from_nanos(10_000_000_000))?;
    assert_eq!(fixture.session.retirement_deadline, fixture.session.shutdown_deadline);
    let _directory = fixture.finish()?;
    Ok(())
}
