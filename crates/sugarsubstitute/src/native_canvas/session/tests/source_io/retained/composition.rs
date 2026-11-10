// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Present real GPU frames through shared composition textures on every enumerated adapter.

use super::*;
use crate::native_canvas::{gpu_policy::EmbeddedGpuPolicy, work::OverviewOriginals};
use sugarsubstitute_desktop::canvas::CanvasContent;
use sugarsubstitute_image_gpu::{AdapterCatalog, AdapterKey, Backend};

const GPU_CAPACITY: u64 = 1024 * 1024 * 1024;

/// Observed renderer kind of one selected frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Kind {
    Cpu,
    Shared,
}

impl Fixture {
    /// Start the embedded host's composition attachment under one explicit renderer policy.
    fn composition(policy: EmbeddedGpuPolicy) -> Result<Self, Box<dyn Error>> {
        let (mut session, clock, directory) = session()?;
        session.config.initial = InitialSource::AwaitHost;
        session.config.gpu = policy.request();
        session.device_pending = session.config.gpu.is_some();
        session.gpu_ready = session.config.gpu.is_none();
        session.attach_composition_surface()?;
        assert_eq!(session.gpu_admitted, session.config.gpu.is_none());
        session.admit_composition_gpu();
        session.input(Input::Drawable(false))?;
        let wake = Arc::new(Wake::default());
        let capacity =
            std::num::NonZeroU64::new(GPU_CAPACITY).ok_or("Invalid GPU execution capacity")?;
        let owner = ExecutionOwner::with_memory_capacity(clock.clone(), wake.clone(), capacity)?;
        let client = owner.client();
        session.start_runtime(Runtime::shared(
            client.clone(),
            ScopeSpec { id: ScopeId::new(1)?, deadline: None },
        )?)?;
        let mut fixture = Self { session, clock, wake, owner, client, _directory: directory };
        fixture.until(|session| session.resources.is_some() && session.renderer_settled())?;
        Ok(fixture)
    }

    /// Deliver real completions and the display cadence until the condition holds.
    fn until(&mut self, condition: impl Fn(&Session) -> bool) -> Result<(), Box<dyn Error>> {
        for _ in 0..2000 {
            if condition(&self.session) {
                return Ok(());
            }
            self.clock.0.fetch_add(20_000_000, Ordering::SeqCst);
            let observed = self.wake.revision();
            self.client.drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
            self.session.poll(Duration::ZERO)?;
            if self.session.failed.is_some() {
                return Err("Canvas failed during composition qualification".into());
            }
            let snapshot = self.session.runtime.as_ref().ok_or("Runtime absent")?.scope.snapshot();
            if snapshot.running + snapshot.queued > 0 && snapshot.terminal_unobserved == 0 {
                self.wake.wait_after(observed);
            }
        }
        let scope = self.session.runtime.as_ref().map(|runtime| runtime.scope.snapshot());
        Err(format!(
            "Composition qualification did not reach its expected state: scope {scope:?},              resources {}, device job {}, device pending {}, gpu admitted {}, gpu ready {},              loading {}, rendering {}, displaying {}, selection {}",
            self.session.resources.is_some(),
            self.session.device_job.is_some(),
            self.session.device_pending,
            self.session.gpu_admitted,
            self.session.gpu_ready,
            self.session.loading.is_some(),
            self.session.rendering.is_some(),
            self.session.displaying.is_some(),
            self.session.selection,
        )
        .into())
    }

    /// Wait for the next unacknowledged selected frame and report its renderer kind.
    fn pending_selected(&mut self) -> Result<(FrameStamp, Kind), Box<dyn Error>> {
        self.until(|session| session.selected_frame().is_some_and(|(frame, _)| frame.pending()))?;
        let (content, _) = self.session.selected_frame().ok_or("Selected frame absent")?;
        let kind = match &content {
            CanvasContent::Cpu(_) => Kind::Cpu,
            CanvasContent::Shared(frame) => {
                let viewport = self.session.document.tools.navigation().viewport().surface();
                assert_eq!(frame.texture.extent(), viewport);
                Kind::Shared
            }
        };
        Ok((content.target(), kind))
    }

    /// Offer one encoded color through the host's ordinary selection path.
    fn select(&mut self, color: [u8; 3]) -> Result<(), Box<dyn Error>> {
        let mut source = Some(self.encoded(color)?);
        assert!(self.session.offer_source(&mut source, CacheReuse::Compatible, None)?);
        self.session.resize(Extent::new(8, 8)?)?;
        self.session.input(Input::Drawable(true))?;
        Ok(())
    }
}

/// Enumerate the hardware Vulkan adapters that can share composition textures.
fn shared_texture_adapters() -> Result<Vec<AdapterKey>, Box<dyn Error>> {
    let clock = Arc::new(TestClock(AtomicU64::new(0)));
    let wake = Arc::new(Wake::default());
    let mut owner = ExecutionOwner::new(clock, wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let spec = JobSpec {
        id: JobId::new(1)?,
        operation: Operation::Infrastructure,
        class: JobClass::IoOrdinary,
        deadline: Some(Moment::from_nanos(30_000_000_000)),
        reservation_bytes: 2 * 1024 * 1024,
        replacement: None,
    };
    let mut job = client.submit_async(&scope, spec, |context| async move {
        let catalog = AdapterCatalog::discover(&context).await?;
        Ok::<_, WorkError>(
            catalog
                .adapters()
                .filter(|adapter| adapter.backend == Backend::Vulkan && adapter.compatible)
                .map(|adapter| (adapter.key(), adapter.shared_textures, adapter.name.clone()))
                .collect::<Vec<_>>(),
        )
    })?;
    let adapters = match wake.completion(&client, &mut job)?.outcome {
        JobOutcome::Success(adapters) => adapters,
        JobOutcome::Failure(cause) => return Err(Box::new(cause)),
        other => return Err(format!("Discovery stopped: {:?}", other.kind()).into()),
    };
    drop(scope);
    wake.finish_shutdown(&mut owner, &client, Moment::from_nanos(10_000_000_000))?;
    let unsupported: Vec<_> =
        adapters.iter().filter(|(_, shared, _)| !shared).map(|(_, _, name)| name).collect();
    if !unsupported.is_empty() {
        return Err(
            format!("Vulkan adapters cannot share composition textures: {unsupported:?}").into()
        );
    }
    if adapters.is_empty() {
        return Err("No compatible Vulkan adapter was enumerated".into());
    }
    Ok(adapters.into_iter().map(|(key, _, _)| key).collect())
}

#[test]
fn each_adapter_policy_presents_shared_frames_and_keeps_previews_on_the_cpu()
-> Result<(), Box<dyn Error>> {
    for key in shared_texture_adapters()? {
        let mut fixture = Fixture::composition(EmbeddedGpuPolicy::Adapter(key))?;
        assert!(fixture.session.gpu_active(), "{key}: {:?}", fixture.session.gpu_fallback());
        assert_eq!(fixture.session.gpu_fallback(), None);
        fixture.select([255, 0, 0])?;
        let (red, kind) = fixture.pending_selected()?;
        assert_eq!(kind, Kind::Shared, "{key}");
        fixture.session.did_present_selected(red)?;
        assert!(fixture.session.did_present_composition(red).is_err());
        assert!(fixture.session.selected_frame().is_some_and(|(frame, _)| {
            !frame.pending() && frame.target() == red && matches!(frame, CanvasContent::Shared(_))
        }));
        assert!(fixture.session.accepted_camera().is_some());

        // Previews are CPU frames on the same surface; withdrawal returns to shared frames.
        let mut preview = Some(fixture.encoded([0, 0, 255])?);
        let deadline = fixture.session.preview_now().checked_add(Duration::from_secs(30))?;
        let token = fixture.session.offer_preview(&mut preview, deadline)?.token();
        assert!(token.is_some(), "{key}: preview refused");
        // Refined final frames remain shared until the preview owns the native slot.
        let offered = loop {
            match fixture.pending_selected()? {
                (target, Kind::Shared) => fixture.session.did_present_selected(target)?,
                (target, Kind::Cpu) => break target,
            }
        };
        fixture.session.did_present_selected(offered)?;
        assert!(fixture.session.preview_shown());
        fixture.session.withdraw_preview()?;
        fixture.until(|session| !session.preview_pending())?;
        let (restored, kind) = fixture.pending_selected()?;
        assert_eq!(kind, Kind::Shared, "{key}");
        assert!(restored.generation > offered.generation);
        fixture.session.did_present_selected(restored)?;

        // A replacement source withdraws its unacknowledged shared frame; a held frame closes.
        fixture.select([0, 255, 0])?;
        let (green, kind) = fixture.pending_selected()?;
        assert_eq!(kind, Kind::Shared, "{key}");
        fixture.session.cancel_selection()?;
        assert!(fixture.session.did_present_composition(green).is_err());
        fixture.select([255, 255, 0])?;
        let (_held, kind) = fixture.pending_selected()?;
        assert_eq!(kind, Kind::Shared, "{key}");
        let _directory = fixture.finish()?;
    }
    Ok(())
}

#[test]
fn overview_grids_render_on_the_gpu_and_resize_withdraws_the_pending_shared_frame()
-> Result<(), Box<dyn Error>> {
    for key in shared_texture_adapters()? {
        let mut fixture = Fixture::composition(EmbeddedGpuPolicy::Adapter(key))?;
        let primary = fixture.encoded([255, 0, 0])?;
        let second = fixture.encoded([0, 0, 255])?;
        let mut originals = Some(crate::native_canvas::work::Originals {
            primary,
            compared: None,
            overview: Some(OverviewOriginals {
                extent: Extent::new(16, 8)?,
                primary_cell: [0.0, 0.0, 8.0, 8.0],
                cells: vec![(second, [8.0, 0.0, 8.0, 8.0])],
            }),
        });
        assert!(fixture.session.offer_originals(&mut originals, CacheReuse::Compatible, None)?);
        fixture.session.resize(Extent::new(16, 8)?)?;
        fixture.session.input(Input::Drawable(true))?;
        let (first, kind) = fixture.pending_selected()?;
        assert_eq!(kind, Kind::Shared, "{key}");
        assert!(
            fixture.session.document.scene.as_ref().is_some_and(|scene| scene.placed.len() == 1)
        );
        fixture.session.resize(Extent::new(12, 6)?)?;
        assert!(fixture.session.did_present_composition(first).is_err());
        let (resized, kind) = fixture.pending_selected()?;
        assert_eq!(kind, Kind::Shared, "{key}");
        fixture.session.did_present_selected(resized)?;
        let _directory = fixture.finish()?;
    }
    Ok(())
}

#[test]
fn unavailable_adapter_policy_falls_back_to_cpu_frames_with_its_reason()
-> Result<(), Box<dyn Error>> {
    let absent = EmbeddedGpuPolicy::Adapter("vulkan:ffff:fffe".parse()?);
    let mut fixture = Fixture::composition(absent)?;
    assert!(!fixture.session.gpu_active());
    assert_eq!(fixture.session.gpu_fallback(), Some("adapter_unavailable"));
    fixture.select([255, 0, 0])?;
    let (red, kind) = fixture.pending_selected()?;
    assert_eq!(kind, Kind::Cpu);
    fixture.session.did_present_selected(red)?;
    let _directory = fixture.finish()?;

    let mut cpu = Fixture::composition(EmbeddedGpuPolicy::Cpu)?;
    assert!(!cpu.session.gpu_active());
    assert_eq!(cpu.session.gpu_fallback(), None);
    cpu.select([0, 255, 0])?;
    assert_eq!(cpu.pending_selected()?.1, Kind::Cpu);
    let _directory = cpu.finish()?;
    Ok(())
}
