// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise cancellation and worker-owned retirement of a host source adapter.

use super::*;
use crate::native_canvas::session::{SelectedOriginals, acquisition::SourceAcquirer};
use std::{
    io,
    path::PathBuf,
    sync::{
        Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use sugarsubstitute_contracts::execution::MemoryLease;

#[derive(Default)]
struct State {
    entered: bool,
    cancelled: bool,
    settled: bool,
    retirements: usize,
}

#[derive(Default)]
struct BlockingSource {
    state: Mutex<State>,
    changed: Condvar,
}

struct ReadySource {
    path: PathBuf,
    settled: AtomicBool,
    close_before_return: bool,
}

impl SourceAcquirer for ReadySource {
    fn reservation_bytes(&self) -> u64 {
        1024
    }

    fn acquire(
        &self,
        _: RuntimeCache,
        _: &MemoryLease,
        _: &JobContext,
    ) -> io::Result<SelectedOriginals> {
        if self.close_before_return {
            self.settled.store(true, Ordering::Release);
        }
        Ok(SelectedOriginals::single(ImageSource::file(self.path.clone())))
    }

    fn cancel(&self) {}

    fn retire(&self, _: &JobContext) -> io::Result<()> {
        self.settled.store(true, Ordering::Release);
        Ok(())
    }

    fn settled(&self) -> bool {
        self.settled.load(Ordering::Acquire)
    }
}

impl SourceAcquirer for BlockingSource {
    fn reservation_bytes(&self) -> u64 {
        1024
    }

    fn acquire(
        &self,
        _: RuntimeCache,
        _: &MemoryLease,
        context: &JobContext,
    ) -> io::Result<SelectedOriginals> {
        assert_eq!(context.spec().class, JobClass::InfrastructureOrdinary);
        let mut state = self.state.lock().map_err(|_| io::Error::other("Source state poisoned"))?;
        state.entered = true;
        self.changed.notify_all();
        drop(crate::completion_wake::wait_while_watched(
            &self.changed,
            state,
            || "the canvas to cancel the blocked source".to_owned(),
            |state| !state.cancelled,
        ));
        Err(io::ErrorKind::Interrupted.into())
    }

    fn cancel(&self) {
        let mut state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        state.cancelled = true;
        self.changed.notify_all();
    }

    fn retire(&self, context: &JobContext) -> io::Result<()> {
        assert_eq!(context.spec().class, JobClass::InfrastructureLifecycle);
        let mut state = self.state.lock().map_err(|_| io::Error::other("Source state poisoned"))?;
        state.retirements += 1;
        state.settled = true;
        Ok(())
    }

    fn settled(&self) -> bool {
        self.state.lock().map(|state| state.settled).unwrap_or(false)
    }
}

fn png(path: &std::path::Path) -> Result<(), Box<dyn Error>> {
    let file = std::fs::File::create(path)?;
    let mut encoder = png::Encoder::new(file, 1, 1);
    encoder.set_color(png::ColorType::Rgb);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(&[255, 0, 0])?;
    writer.finish()?;
    Ok(())
}

#[test]
fn acquired_original_is_imported_and_becomes_the_selected_canvas_image()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    let path = fixture._directory.path().join("acquired.png");
    png(&path)?;
    let owner =
        Arc::new(ReadySource { path, settled: AtomicBool::new(false), close_before_return: true });
    let mut source: Option<Arc<dyn SourceAcquirer>> = Some(owner.clone());
    assert!(fixture.session.offer_acquirer(&mut source, CacheReuse::Compatible)?);
    fixture.finish_loading()?;
    assert!(owner.settled());
    let mut raster = fixture
        .session
        .document
        .scene
        .as_ref()
        .and_then(|scene| scene.source.clone())
        .ok_or("Acquired raster is absent")?;
    let scope = &fixture.session.runtime.as_ref().ok_or("Canvas scope absent")?.scope;
    let spec = fixture.session.spec(
        Operation::Infrastructure,
        JobClass::InfrastructureOrdinary,
        1024 * 1024,
        Duration::from_secs(30),
        None,
    )?;
    let mut job = fixture.client.submit_blocking(scope, spec, move |context| {
        let descriptor = raster.descriptor();
        let mut pixel = [Pixel::TRANSPARENT];
        raster.read(
            descriptor.asset(),
            descriptor.layout().extent().region(),
            &mut pixel,
            &context,
        )?;
        Ok::<_, sugarsubstitute_image_ops::ImageError>(pixel[0])
    })?;
    let result = fixture.wake.completion(&fixture.client, &mut job)?;
    assert!(
        matches!(result.outcome, JobOutcome::Success(pixel) if pixel == Pixel::linear(1.0, 0.0, 0.0, 1.0)?)
    );
    drop(result.memory);
    drop(job);
    let _directory = fixture.finish()?;
    Ok(())
}

#[test]
fn unretired_acquisition_cannot_publish_even_when_decoding_succeeds() -> Result<(), Box<dyn Error>>
{
    let mut fixture = Fixture::new()?;
    let path = fixture._directory.path().join("unretired.png");
    png(&path)?;
    let owner =
        Arc::new(ReadySource { path, settled: AtomicBool::new(false), close_before_return: false });
    let mut source: Option<Arc<dyn SourceAcquirer>> = Some(owner.clone());
    assert!(fixture.session.offer_acquirer(&mut source, CacheReuse::Compatible)?);
    loop {
        let revision = fixture.wake.revision();
        fixture.client.drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
        fixture.session.poll(Duration::ZERO)?;
        if fixture.session.failed.is_some()
            && fixture.session.loading.is_none()
            && fixture.session.acquisition_retirement.is_none()
            && fixture.session.pending_retirement.is_none()
            && fixture.session.retirement.is_none()
        {
            break;
        }
        fixture.wake.wait_after(revision);
    }
    assert!(owner.settled());
    assert!(fixture.session.document.scene.as_ref().is_some_and(|scene| scene.source.is_none()));
    assert_eq!(
        fixture.session.failed.take().ok_or("Expected acquisition failure")?.to_string(),
        "Acquisition returned before native retirement"
    );
    let _directory = fixture.finish()?;
    Ok(())
}

#[test]
fn revoked_source_waits_for_real_job_completion_and_worker_retirement() -> Result<(), Box<dyn Error>>
{
    let mut fixture = Fixture::new()?;
    let owner = Arc::new(BlockingSource::default());
    let mut source: Option<Arc<dyn SourceAcquirer>> = Some(owner.clone());
    assert!(fixture.session.offer_acquirer(&mut source, CacheReuse::Compatible)?);
    while fixture.session.loading.is_none() {
        let revision = fixture.wake.revision();
        fixture.client.drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
        fixture.session.poll(Duration::ZERO)?;
        if fixture.session.loading.is_none() {
            fixture.wake.wait_after(revision);
        }
    }
    let state = owner.state.lock().map_err(|_| "Source state poisoned")?;
    drop(crate::completion_wake::wait_while_watched(
        &owner.changed,
        state,
        || "the blocked source acquisition to start".to_owned(),
        |state| !state.entered,
    ));
    let selection = fixture.session.selection;
    fixture.session.revoke_acquirer()?;
    assert!(fixture.session.selection > selection);
    loop {
        let revision = fixture.wake.revision();
        fixture.client.drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
        fixture.session.poll(Duration::ZERO)?;
        if fixture.session.loading.is_none() && fixture.session.acquisition_retirement.is_none() {
            break;
        }
        fixture.wake.wait_after(revision);
    }
    let state = owner.state.lock().map_err(|_| "Source state poisoned")?;
    assert!(state.cancelled && state.settled);
    assert_eq!(state.retirements, 1);
    drop(state);
    assert!(fixture.session.document.scene.as_ref().is_some_and(|scene| scene.source.is_none()));
    let _directory = fixture.finish()?;
    Ok(())
}
