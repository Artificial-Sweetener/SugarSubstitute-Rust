// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify that reselecting a retained output presents its finished frame without new work.

use super::*;
use crate::native_canvas::session::{RetentionKey, SelectedOriginals, acquisition::SourceAcquirer};
use std::io;
use sugarsubstitute_canvas::NavigationCommand;

mod budget;

/// Enough retained-document storage for every 8x8 test document these tests park.
const FUNDED: u64 = 8 * 1024 * 1024;

/// One frame acknowledged on the native CPU path, with its centre pixel.
#[derive(Clone, Copy)]
struct Shown {
    evidence: DisplayEvidence,
    pixel: u32,
}

/// A host acquisition that blocks until the canvas cancels it, as a stalled download does.
#[derive(Default)]
struct HeldSource {
    state: Mutex<HeldState>,
    changed: Condvar,
}

#[derive(Default)]
struct HeldState {
    entered: bool,
    cancelled: bool,
    retirements: usize,
}

impl HeldSource {
    fn state(&self) -> std::sync::MutexGuard<'_, HeldState> {
        self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Wait until a storage worker actually entered the acquisition.
    fn wait_entered(&self) -> Result<(), Box<dyn Error>> {
        let (state, timeout) = self
            .changed
            .wait_timeout_while(self.state(), Duration::from_secs(10), |state| !state.entered)
            .map_err(|_| "Held source state poisoned")?;
        if timeout.timed_out() && !state.entered {
            drop(state);
            self.cancel();
            return Err("Held acquisition did not start".into());
        }
        Ok(())
    }
}

impl SourceAcquirer for HeldSource {
    fn reservation_bytes(&self) -> u64 {
        1024
    }

    fn acquire(
        &self,
        _: RuntimeCache,
        _: &MemoryLease,
        _: &JobContext,
    ) -> io::Result<SelectedOriginals> {
        let mut state = self.state.lock().map_err(|_| io::Error::other("Held state poisoned"))?;
        state.entered = true;
        self.changed.notify_all();
        while !state.cancelled {
            state =
                self.changed.wait(state).map_err(|_| io::Error::other("Held state poisoned"))?;
        }
        Err(io::ErrorKind::Interrupted.into())
    }

    fn cancel(&self) {
        self.state().cancelled = true;
        self.changed.notify_all();
    }

    fn retire(&self, _: &JobContext) -> io::Result<()> {
        self.state().retirements += 1;
        Ok(())
    }

    fn settled(&self) -> bool {
        self.state().retirements > 0
    }
}

fn key(name: &str) -> RetentionKey {
    RetentionKey::new(name)
}

/// Whether the named channel dominates a presented pixel, which may blend the image with the
/// canvas matte at its edges.
fn dominant(pixel: u32, shift: u32) -> bool {
    let channel = |shift: u32| (pixel >> shift) & 0xff;
    let value = channel(shift);
    value > 0x60
        && [16, 8, 0]
            .into_iter()
            .filter(|&other| other != shift)
            .all(|other| value > 2 * channel(other))
}

fn red(pixel: u32) -> bool {
    dominant(pixel, 16)
}

fn green(pixel: u32) -> bool {
    dominant(pixel, 8)
}

impl Fixture {
    /// Start an attached 8x8 canvas that funds retained documents.
    fn retaining() -> Result<Self, Box<dyn Error>> {
        let mut fixture = Self::funded(true, FUNDED)?;
        fixture.session.resize(Extent::new(8, 8)?)?;
        fixture.session.input(Input::Drawable(true))?;
        Ok(fixture)
    }

    /// Whether a job or worker-owned disposal still supplies a completion wake.
    fn busy(&self) -> bool {
        self.session.loading.is_some()
            || self.session.rendering.is_some()
            || self.session.displaying.is_some()
            || self.session.retirement.is_some()
            || self.session.acquisition_retirement.is_some()
    }

    /// Deliver completions and acknowledge one pending frame, or advance injected time when
    /// nothing is running so pacing deadlines elapse.
    fn step(&mut self) -> Result<Option<Shown>, Box<dyn Error>> {
        let observed = self.wake.revision();
        self.client.drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
        self.session.poll(Duration::ZERO)?;
        if self.session.failed.is_some() {
            return Err("Canvas failed while settling retained documents".into());
        }
        if let Some(frame) = self.session.frame().filter(|frame| frame.pending) {
            let target = frame.target;
            let extent = frame.damage.dirty().surface();
            let center = u64::from(extent.height() / 2) * u64::from(extent.width())
                + u64::from(extent.width() / 2);
            let pixel = *frame.pixels.get(usize::try_from(center)?).ok_or("Centre absent")?;
            let evidence =
                self.session.document.pending_cpu_evidence.ok_or("Pending evidence absent")?;
            self.session.did_present_cpu(target)?;
            return Ok(Some(Shown { evidence, pixel }));
        }
        let progress = self.owner.poll_shutdown()?;
        let undelivered = progress
            .unresolved_jobs
            .iter()
            .any(|job| !job.acknowledged && matches!(job.phase, JobPhase::Terminal(_)));
        if progress.running + progress.queued > 0 && !undelivered {
            self.wake.wait_after(observed)?;
        } else {
            self.clock.0.fetch_add(100_000_000, Ordering::SeqCst);
        }
        Ok(None)
    }

    /// Step until the next acknowledged frame, checking `invariant` after every step.
    fn next_shown(
        &mut self,
        invariant: impl Fn(&Session) -> Result<(), &'static str>,
    ) -> Result<Shown, Box<dyn Error>> {
        for _ in 0..1000 {
            let shown = self.step()?;
            invariant(&self.session)?;
            if let Some(shown) = shown {
                return Ok(shown);
            }
        }
        Err(format!(
            "No frame was presented (restore={}, loading={}, rendering={}, displaying={}, \
             acquisition={}, retirement={}, selection={}, scene={:?})",
            self.session.pending_restore.is_some(),
            self.session.loading.is_some(),
            self.session.rendering.is_some(),
            self.session.displaying.is_some(),
            self.session.acquisition_retirement.is_some(),
            self.session.retirement.is_some(),
            self.session.selection,
            self.session.document.scene.as_ref().map(|scene| scene.source_revision),
        )
        .into())
    }

    /// Whether the current selection's full-quality detail is native-accepted and idle.
    fn detail_settled(&self) -> bool {
        let session = &self.session;
        session.document.display_evidence.is_some_and(|evidence| {
            evidence.tier == BackingTier::Detail
                && evidence.source_revision == session.selection
                && session.document.native_presented == Some(evidence.target)
        }) && session.pending_restore.is_none()
            && session.document.cpu_pending.is_none()
            && !self.busy()
            && !session.document.tools.navigation().interacting()
            && session.document.tools.navigation().viewport()
                == session.document.tools.navigation().target_viewport()
    }

    /// Step until the current selection's detail is shown and its work has settled.
    fn settle(&mut self) -> Result<(), Box<dyn Error>> {
        for _ in 0..1000 {
            if self.detail_settled() {
                return Ok(());
            }
            self.step()?;
        }
        let session = &self.session;
        Err(format!(
            "Selected detail never settled (evidence={:?}, selection={}, presented={:?}, \
             restore={}, busy={}, retained={}, open={}, pending={}, deferred={}, jobs={:?})",
            session.document.display_evidence.map(|evidence| (
                evidence.tier,
                evidence.source_revision,
                session.document.native_presented == Some(evidence.target)
            )),
            session.selection,
            session.document.presented.map(|shown| (shown.tier, shown.source_revision)),
            session.pending_restore.is_some(),
            self.busy(),
            session.retained.count(),
            session.pending_open.is_some(),
            session.pending_retirement.is_some(),
            session.deferred_retirement.is_some(),
            self.owner.poll_shutdown().map(|progress| progress
                .unresolved_jobs
                .iter()
                .map(|job| (job.spec.operation, job.phase, job.spec.reservation_bytes))
                .collect::<Vec<_>>()),
        )
        .into())
    }

    /// Publish a small encoded output whose original is funded by the canvas admission, as an
    /// acquired output is, rather than pinning its publishing job's reservation.
    fn output(&mut self, color: [u8; 3]) -> Result<ImageSource, Box<dyn Error>> {
        let resources = self.session.resources.as_ref().ok_or("Canvas resources absent")?;
        let (cache, memory) = (resources.cache.clone(), resources.memory.clone());
        let scope = &self.session.runtime.as_ref().ok_or("Canvas scope absent")?.scope;
        let spec = self.session.spec(
            Operation::Infrastructure,
            JobClass::InfrastructureOrdinary,
            8 * 1024 * 1024,
            Duration::from_secs(30),
            None,
        )?;
        let mut job = self.client.submit_blocking(scope, spec, move |context| {
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
                &context,
            )?;
            entry.write("output.png", &bytes, &context)?;
            entry.publish(&context)?;
            let mut input = Some(entry.into_file_reader("output.png", &context)?);
            Ok::<_, WorkError>(ImageSource::encoded(&mut input, &memory)?)
        })?;
        match self.wake.completion(&self.client, &mut job)?.outcome {
            JobOutcome::Success(source) => Ok(source),
            JobOutcome::Failure(cause) => Err(Box::new(cause)),
            other => Err(format!("Encoded output stopped: {:?}", other.kind()).into()),
        }
    }

    /// Show `color` as output `name` until its full-quality detail is native-accepted.
    fn show(&mut self, color: [u8; 3], name: &str) -> Result<(), Box<dyn Error>> {
        let mut source = Some(self.output(color)?);
        assert!(self.session.offer_source(&mut source, CacheReuse::Compatible, None)?);
        self.session.retain_selection(key(name));
        self.settle()
    }

    /// Step until worker-owned disposal of everything leaving retention has completed.
    fn settle_retirement(&mut self) -> Result<(), Box<dyn Error>> {
        for _ in 0..1000 {
            if !self.busy()
                && self.session.pending_retirement.is_none()
                && self.session.deferred_retirement.is_none()
                && !self.session.retained.retiring_pending()
            {
                return Ok(());
            }
            self.step()?;
        }
        Err("Retained document retirement never settled".into())
    }

    /// Unclaimed bytes of the session's retained admission.
    fn available(&self) -> Result<u64, Box<dyn Error>> {
        let memory = &self.session.resources.as_ref().ok_or("Canvas resources absent")?.memory;
        Ok(memory.claim(u64::MAX).err().ok_or("Unbounded claim succeeded")?.available_bytes)
    }
}

#[test]
fn reselecting_a_retained_output_presents_its_finished_detail_at_its_camera_without_new_work()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::retaining()?;
    fixture.show([255, 0, 0], "a")?;
    let fit = fixture.session.document.tools.navigation().viewport();
    assert!(matches!(
        fixture.session.navigate(NavigationCommand::OneToOne)?,
        CameraCommandResult::Accepted(_)
    ));
    fixture.settle()?;
    let zoomed = fixture.session.document.tools.navigation().viewport();
    assert_ne!(zoomed, fit);

    fixture.show([0, 255, 0], "b")?;
    assert_eq!(fixture.session.document.tools.navigation().viewport(), fit);
    assert!(fixture.session.retained.contains(&key("a")));

    assert!(fixture.session.reselect_retained(&key("a"))?);
    let selection = fixture.session.selection;
    let shown = fixture.next_shown(|session| {
        if session.loading.is_some() {
            return Err("Reselection loaded its source again");
        }
        if session.rendering.is_some() {
            return Err("Reselection prepared its frame again");
        }
        Ok(())
    })?;
    assert_eq!(shown.evidence.tier, BackingTier::Detail);
    assert_eq!(shown.evidence.source_revision, selection);
    assert!(red(shown.pixel), "{:x}", shown.pixel);
    let camera = fixture.session.accepted_camera().ok_or("Reselected camera absent")?;
    assert_eq!(camera.viewport, zoomed);
    assert_eq!(camera.source_revision, selection);
    assert!(fixture.session.retained.contains(&key("b")));
    assert!(!fixture.session.retained.contains(&key("a")));
    fixture.settle()?;
    let _directory = fixture.finish()?;
    Ok(())
}

#[test]
fn returning_while_another_output_downloads_relabels_the_current_document()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::retaining()?;
    fixture.show([255, 0, 0], "a")?;
    let held = Arc::new(HeldSource::default());
    let mut source: Option<Arc<dyn SourceAcquirer>> = Some(held.clone());
    assert!(fixture.session.offer_acquirer(&mut source, CacheReuse::Compatible)?);
    fixture.session.retain_selection(key("b"));
    fixture.session.poll(Duration::ZERO)?;
    assert!(fixture.session.loading.is_some(), "The held acquisition was not admitted");
    held.wait_entered()?;

    assert!(fixture.session.reselect_retained(&key("a"))?);
    assert!(held.state().cancelled);
    let selection = fixture.session.selection;
    let shown = fixture.next_shown(|session| {
        session.rendering.is_none().then_some(()).ok_or("Relabelled output prepared again")
    })?;
    assert_eq!(shown.evidence.tier, BackingTier::Detail);
    assert_eq!(shown.evidence.source_revision, selection);
    assert!(red(shown.pixel), "{:x}", shown.pixel);
    fixture.settle()?;
    fixture.settle_retirement()?;
    assert_eq!(held.state().retirements, 1);
    assert_eq!(fixture.session.document.retention, Some(key("a")));
    assert_eq!(fixture.session.retained.count(), 0);
    let _directory = fixture.finish()?;
    Ok(())
}

#[test]
fn returning_while_another_output_renders_restores_after_it_ends_without_showing_it()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::retaining()?;
    fixture.show([255, 0, 0], "a")?;
    fixture.show([0, 0, 255], "b")?;
    let mut green_source = Some(fixture.output([0, 255, 0])?);
    assert!(fixture.session.offer_source(&mut green_source, CacheReuse::Compatible, None)?);
    fixture.session.retain_selection(key("c"));
    for _ in 0..1000 {
        if fixture.session.rendering.is_some() {
            break;
        }
        assert!(fixture.step()?.is_none_or(|shown| !green(shown.pixel)));
    }
    assert!(fixture.session.rendering.is_some(), "The replacement output never rendered");

    assert!(fixture.session.reselect_retained(&key("a"))?);
    let selection = fixture.session.selection;
    let shown = fixture.next_shown(|_| Ok(()))?;
    assert_eq!(shown.evidence.source_revision, selection);
    assert_eq!(shown.evidence.tier, BackingTier::Detail);
    assert!(red(shown.pixel), "{:x}", shown.pixel);
    assert!(!fixture.session.retained.contains(&key("c")));
    assert!(fixture.session.retained.contains(&key("b")));
    fixture.settle()?;
    let _directory = fixture.finish()?;
    Ok(())
}

#[test]
fn comparison_overview_and_preview_documents_are_never_retained() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::retaining()?;
    let mut compared = Some(SelectedOriginals {
        primary: fixture.output([255, 0, 0])?,
        compared: Some(fixture.output([0, 255, 0])?),
        overview: None,
    });
    assert!(fixture.session.offer_originals(&mut compared, CacheReuse::Compatible, None)?);
    fixture.session.retain_selection(key("comparison"));
    fixture.settle()?;
    let mut overview = Some(SelectedOriginals {
        primary: fixture.output([0, 0, 255])?,
        compared: None,
        overview: Some(crate::native_canvas::session::OverviewOriginals {
            extent: Extent::new(8, 8)?,
            primary_cell: [0.0, 0.0, 4.0, 8.0],
            cells: vec![(fixture.output([255, 255, 0])?, [4.0, 0.0, 4.0, 8.0])],
        }),
    });
    assert!(fixture.session.offer_originals(&mut overview, CacheReuse::Compatible, None)?);
    fixture.session.retain_selection(key("overview"));
    fixture.settle()?;
    fixture.show([255, 0, 255], "single")?;
    assert_eq!(fixture.session.retained.count(), 0);

    let deadline = fixture.session.preview_now().checked_add(Duration::from_secs(30))?;
    let mut preview = Some(fixture.output([0, 255, 255])?);
    assert!(fixture.session.offer_preview(&mut preview, deadline)?.token().is_some());
    assert!(fixture.session.preview_pending());
    // Preview custody owns presentation, so even the current output must be acquired again.
    assert!(!fixture.session.reselect_retained(&key("single"))?);
    fixture.session.withdraw_preview()?;
    let _directory = fixture.finish()?;
    Ok(())
}

#[test]
fn closing_with_retained_documents_retires_them_and_settles() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::retaining()?;
    fixture.show([255, 0, 0], "a")?;
    let backing = fixture
        .session
        .document
        .front
        .as_ref()
        .map(Arc::downgrade)
        .ok_or("Detail backing absent")?;
    fixture.show([0, 255, 0], "b")?;
    fixture.show([0, 0, 255], "c")?;
    assert_eq!(fixture.session.retained.count(), 2);
    assert!(backing.upgrade().is_some());
    let _directory = fixture.finish()?;
    assert!(backing.upgrade().is_none(), "Close kept a retained backing");
    Ok(())
}

#[test]
fn a_resize_between_parking_and_return_still_proves_complete_coverage() -> Result<(), Box<dyn Error>>
{
    let mut fixture = Fixture::retaining()?;
    fixture.show([255, 0, 0], "a")?;
    fixture.show([0, 255, 0], "b")?;
    fixture.session.resize(Extent::new(10, 8)?)?;
    fixture.settle()?;
    assert!(fixture.session.reselect_retained(&key("a"))?);
    let selection = fixture.session.selection;
    let shown = fixture.next_shown(|_| Ok(()))?;
    assert_eq!(shown.evidence.source_revision, selection);
    assert_eq!(shown.evidence.view.surface(), Extent::new(10, 8)?);
    assert!(red(shown.pixel), "{:x}", shown.pixel);
    fixture.settle()?;
    let camera = fixture.session.accepted_camera().ok_or("Resized camera absent")?;
    assert_eq!(camera.viewport.surface(), Extent::new(10, 8)?);
    let _directory = fixture.finish()?;
    Ok(())
}

#[test]
fn forgetting_retained_documents_returns_their_memory() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::retaining()?;
    fixture.show([255, 0, 0], "a")?;
    fixture.show([0, 255, 0], "b")?;
    fixture.show([0, 0, 255], "c")?;
    fixture.settle_retirement()?;
    let retained = fixture.session.retained.bytes();
    assert_eq!(fixture.session.retained.count(), 2);
    let before = fixture.available()?;
    fixture.session.forget_retained();
    assert_eq!(fixture.session.retained.count(), 0);
    assert!(!fixture.session.reselect_retained(&key("a"))?);
    fixture.settle_retirement()?;
    assert!(!fixture.session.retained.has_resources());
    assert!(fixture.available()? >= before + retained);
    let _directory = fixture.finish()?;
    Ok(())
}
