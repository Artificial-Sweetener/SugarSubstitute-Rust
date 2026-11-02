// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own real encoded input, shared execution and native acknowledgements for viewport regressions.

use super::*;

/// Baseline JFIF 4:4:4 sample of flat sRGB `(128, 64, 32)`, 19x17 pixels.
const FLAT_JPEG: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../sugarsubstitute-image-io/src/tests/jpeg/fixtures/444.jpg"
));
/// Progressive JPEG, outside the decoder's baseline single-scan subset.
const PROGRESSIVE_JPEG: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../sugarsubstitute-image-io/src/tests/jpeg/fixtures/progressive.jpg"
));

/// Execution progress observed between host-loop turns.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::startup::canvas::tests::surface) enum Execution {
    /// Admitted work is queued or running; its completion supplies a wake.
    Running,
    /// A terminal completion has not been taken by its owner; its wake may still be in flight.
    Delivering,
    /// No admitted work remains, so only host input or a reported deadline can make progress.
    Idle,
}

pub(in crate::startup::canvas::tests::surface) struct Fixture {
    pub(in crate::startup::canvas::tests::surface) canvas: EmbeddedCanvas,
    pub(in crate::startup::canvas::tests::surface) clock: Arc<ClockValue>,
    pub(in crate::startup::canvas::tests::surface) panel: PanelSession,
    pub(in crate::startup::canvas::tests::surface) memory: MemoryLease,
    pub(in crate::startup::canvas::tests::surface) identity: MainIdentity,
    pub(in crate::startup::canvas::tests::surface) client: ExecutionClient,
    pub(in crate::startup::canvas::tests::surface) wake: Arc<Wake>,
    owner: ExecutionOwner,
    allocation: WebViewAllocation,
    directory: tempfile::TempDir,
}

/// Admitted work of one execution scope, independent of other scopes sharing the owner.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::startup::canvas::tests::surface) struct ScopeWork {
    /// Accepted work that has not started; it may wait behind another scope's lane.
    pub(in crate::startup::canvas::tests::surface) queued: usize,
    /// Started work whose terminal outcome supplies a wake.
    pub(in crate::startup::canvas::tests::surface) running: usize,
    /// Terminal outcomes that their owner has not taken.
    pub(in crate::startup::canvas::tests::surface) undelivered: usize,
}

impl Fixture {
    /// Select an actual source on a prepared visible attachment, leaving commit to each test.
    pub(in crate::startup::canvas::tests::surface) fn new() -> Result<Self, Box<dyn Error>> {
        Self::with_density(1.0)
    }

    /// Project CSS geometry through the same trusted desktop allocation density as production.
    pub(in crate::startup::canvas::tests::surface) fn with_density(
        density: f64,
    ) -> Result<Self, Box<dyn Error>> {
        Self::with_options(density, true, None)
    }

    /// Prepare visible geometry without selecting any final result.
    pub(in crate::startup::canvas::tests::surface) fn empty() -> Result<Self, Box<dyn Error>> {
        Self::with_options(1.0, false, None)
    }

    /// Prepare visible geometry whose selections fetch through the authenticated Comfy backend.
    pub(in crate::startup::canvas::tests::surface) fn comfy(
        backend: crate::startup::CanvasBackend,
    ) -> Result<Self, Box<dyn Error>> {
        Self::with_options(1.0, false, Some(backend))
    }

    /// Share actual execution and attachment setup across final and first-preview qualification.
    fn with_options(
        density: f64,
        select: bool,
        backend: Option<crate::startup::CanvasBackend>,
    ) -> Result<Self, Box<dyn Error>> {
        let directory = tempfile::tempdir()?;
        let output = directory.path().join("canvas-qualification-output");
        std::fs::create_dir(&output)?;
        image(&output.join("wide.png"), 320, 160, [255, 0, 0])?;
        image(&output.join("tall.png"), 160, 320, [0, 0, 255])?;
        image(&output.join("green.png"), 320, 160, [0, 255, 0])?;
        for (filename, bytes) in [("flat.jpg", FLAT_JPEG), ("progressive.jpg", PROGRESSIVE_JPEG)] {
            std::io::Write::write_all(&mut std::fs::File::create(output.join(filename))?, bytes)?;
        }
        let clock = Arc::new(ClockValue(AtomicU64::new(0)));
        let wake = Arc::new(Wake::default());
        let owner = ExecutionOwner::new(clock.clone(), wake.clone())?;
        let client = owner.client();
        let mut canvas = EmbeddedCanvas::new(
            directory.path().to_path_buf(),
            DiagnosticHealth::fixture(),
            clock.clone(),
            JobSequence::new(),
            ScopeSpec { id: ScopeId::new(1)?, deadline: None },
            funding()?,
        );
        if backend.is_some() {
            canvas.surface = crate::startup::canvas::surface::NativeSurface::new();
        }
        canvas.start(
            client.clone(),
            "en",
            crate::native_canvas::gpu_policy::EmbeddedGpuPolicy::Cpu,
        )?;
        while !canvas.reported_ready {
            let observed = wake.revision();
            pump(&mut canvas, &client)?;
            if !canvas.reported_ready {
                wake.wait_after(observed)?;
            }
        }
        let identity = identity()?;
        canvas.backend(backend)?;
        let mut fixture = Self {
            canvas,
            clock,
            panel: PanelSession::for_main(identity)?,
            memory: MemoryLease::new(128 * 1024, Box::new(Release)),
            identity,
            client,
            wake,
            owner,
            allocation: WebViewAllocation {
                bounds: PhysicalRect { x: 0, y: 0, width: 300, height: 150 },
                physical_pixels_per_css_pixel: density,
            },
            directory,
        };
        fixture.send(1, "surface.prepare", json!({}))?;
        wait_reply(&mut fixture.canvas, &fixture.client, &fixture.wake, 1, "ready")?;
        fixture.geometry(2, 80, 40)?;
        fixture.take(2, "applied")?;
        if select {
            fixture.select(3, "wide.png")?;
            wait_reply(&mut fixture.canvas, &fixture.client, &fixture.wake, 3, "applied")?;
        }
        Ok(fixture)
    }

    /// Submit the same closed public envelope used by the shared canvas client.
    pub(in crate::startup::canvas::tests::surface) fn send(
        &mut self,
        id: u64,
        kind: &str,
        extra: Value,
    ) -> Result<(), Box<dyn Error>> {
        self.canvas.surface(
            self.identity,
            request(&self.panel, id, kind, extra)?,
            (kind == "surface.geometry").then_some(self.allocation),
            self.memory.claim(64 * 1024)?,
        )
    }

    /// Replace the desktop-observed browser allocation attached to later geometry, as a window
    /// resize or DPI change does.
    pub(in crate::startup::canvas::tests::surface) fn reallocate(
        &mut self,
        allocation: WebViewAllocation,
    ) {
        self.allocation = allocation;
    }

    pub(in crate::startup::canvas::tests::surface) fn viewport(
        &mut self,
        id: u64,
        command: Value,
    ) -> Result<(), Box<dyn Error>> {
        self.send(id, "surface.viewport.request", json!({"command":command}))
    }

    pub(in crate::startup::canvas::tests::surface) fn geometry(
        &mut self,
        id: u64,
        width: u32,
        height: u32,
    ) -> Result<(), Box<dyn Error>> {
        self.send(
            id,
            "surface.geometry",
            json!({"geometry":{
                "coordinateSpace":"client", "units":"css-pixels", "origin":{"x":10,"y":20},
                "viewport":{"width":width,"height":height},
                "clip":{"x":10,"y":20,"width":width,"height":height},
                "physicalPixelsPerCssPixel":self.allocation.physical_pixels_per_css_pixel,
                "visible":true, "focused":false
            }}),
        )
    }

    pub(in crate::startup::canvas::tests::surface) fn select(
        &mut self,
        id: u64,
        filename: &str,
    ) -> Result<(), Box<dyn Error>> {
        self.send(
            id,
            "surface.select",
            json!({
                "itemId":format!("result:{filename}"),
                "file":{"filename":filename,"subfolder":"","type":"output"}
            }),
        )
    }

    pub(in crate::startup::canvas::tests::surface) fn cancel(
        &mut self,
        id: u64,
    ) -> Result<(), Box<dyn Error>> {
        let message = CanvasSurfaceMessage::decode(&serde_json::to_vec(&json!({
            "protocol":"sugarcubes.canvas", "version":3, "session":self.panel.as_str(),
            "requestId":id, "type":"cancel"
        }))?)?;
        self.canvas.surface(self.identity, message, None, self.memory.claim(64 * 1024)?)
    }

    pub(in crate::startup::canvas::tests::surface) fn advance(&self, milliseconds: u64) {
        self.clock.0.fetch_add(milliseconds * 1_000_000, Ordering::SeqCst);
    }

    pub(in crate::startup::canvas::tests::surface) fn pump(
        &mut self,
    ) -> Result<(), Box<dyn Error>> {
        pump(&mut self.canvas, &self.client)
    }

    /// Observe real worker progress before advancing injected presentation cadence.
    pub(in crate::startup::canvas::tests::surface) fn wait_preview_progress(
        &self,
        observed: u64,
    ) -> Result<(), Box<dyn Error>> {
        let progress = self.owner.poll_shutdown()?;
        let terminal = progress.unresolved_jobs.iter().any(|job| {
            !job.acknowledged
                && matches!(job.phase, sugarsubstitute_contracts::execution::JobPhase::Terminal(_))
        });
        if progress.running + progress.queued > 0 && !terminal {
            self.wake.wait_after(observed).map_err(|cause| {
                format!("Preview worker progress stopped: {cause}; {progress:?}")
            })?;
        } else if self.canvas.next_deadline().is_some() {
            self.advance(100);
        } else if !terminal {
            return Err("Preview has no worker or presentation deadline to advance".into());
        }
        Ok(())
    }

    /// Classify actual execution progress that can still arrive without new host input.
    pub(in crate::startup::canvas::tests::surface) fn execution(
        &self,
    ) -> Result<Execution, Box<dyn Error>> {
        let progress = self.owner.poll_shutdown()?;
        if progress.running + progress.queued > 0 {
            return Ok(Execution::Running);
        }
        let delivering = progress.unresolved_jobs.iter().any(|job| {
            !job.acknowledged
                && matches!(job.phase, sugarsubstitute_contracts::execution::JobPhase::Terminal(_))
        });
        Ok(if delivering { Execution::Delivering } else { Execution::Idle })
    }

    /// Classify one scope's admitted work, so other scopes sharing this owner can hold its lanes.
    pub(in crate::startup::canvas::tests::surface) fn scope_work(
        &self,
        scope: ScopeId,
    ) -> Result<ScopeWork, Box<dyn Error>> {
        let progress = self.owner.poll_shutdown()?;
        let mut work = ScopeWork::default();
        for job in progress.unresolved_jobs.iter().filter(|job| job.scope == scope) {
            match job.phase {
                sugarsubstitute_contracts::execution::JobPhase::Queued => work.queued += 1,
                sugarsubstitute_contracts::execution::JobPhase::Running => work.running += 1,
                sugarsubstitute_contracts::execution::JobPhase::Terminal(_)
                    if !job.acknowledged =>
                {
                    work.undelivered += 1;
                }
                sugarsubstitute_contracts::execution::JobPhase::Terminal(_) => {}
            }
        }
        Ok(work)
    }

    /// Describe one scope's unresolved jobs for a stall diagnostic.
    pub(in crate::startup::canvas::tests::surface) fn scope_jobs(
        &self,
        scope: ScopeId,
    ) -> Result<String, Box<dyn Error>> {
        let progress = self.owner.poll_shutdown()?;
        Ok(progress
            .unresolved_jobs
            .iter()
            .filter(|job| job.scope == scope)
            .map(|job| {
                format!(
                    "{:?}/{:?}/{:?}/ack={}/{}B",
                    job.spec.operation,
                    job.spec.class,
                    job.phase,
                    job.acknowledged,
                    job.spec.reservation_bytes
                )
            })
            .collect::<Vec<_>>()
            .join(", "))
    }

    /// Report later geometry inside different trusted web view bounds at the same density.
    pub(in crate::startup::canvas::tests::surface) fn rebound(&mut self, bounds: PhysicalRect) {
        self.reallocate(WebViewAllocation { bounds, ..self.allocation });
    }

    /// Write another qualification output beside the fixture's images.
    pub(in crate::startup::canvas::tests::surface) fn output(&self, filename: &str) -> PathBuf {
        self.directory.path().join("canvas-qualification-output").join(filename)
    }

    /// Present every pending frame until the shown native pixels satisfy `accept`, given the
    /// row-major pixels and frame width; returns the accepted pixels.
    pub(in crate::startup::canvas::tests::surface) fn settle_pixels(
        &mut self,
        accept: impl Fn(&[u32], u32) -> bool,
    ) -> Result<Vec<u32>, Box<dyn Error>> {
        let mut last = Vec::new();
        for _ in 0..500 {
            let observed = self.wake.revision();
            self.advance(100);
            self.pump()?;
            if let Some(frame) = prepared_frame(&self.canvas) {
                let width = frame.placement.viewport.width;
                let target = frame.content.target();
                let pending = frame.content.pending();
                last = frame.content.cpu_pixels().to_vec();
                if pending {
                    // Acknowledge, then re-read the same frame before waiting for further work.
                    self.canvas.presented(target)?;
                    continue;
                }
                if accept(&last, width) {
                    return Ok(last);
                }
            }
            self.wait_preview_progress(observed).map_err(|cause| {
                format!("{cause}; last row {:x?}", last.get(..80).unwrap_or_default())
            })?;
        }
        Err(format!("Native pixels never settled; last row {:x?}", last.get(..80)).into())
    }
    pub(in crate::startup::canvas::tests::surface) fn accepted(
        &self,
    ) -> Result<AcceptedCamera, Box<dyn Error>> {
        self.canvas
            .session
            .as_ref()
            .and_then(Session::accepted_camera)
            .ok_or("Native-accepted camera is absent".into())
    }

    /// Borrow only an actual pending frame; acknowledged retained frames cannot free a new slot.
    pub(in crate::startup::canvas::tests::surface) fn pending(
        &mut self,
    ) -> Result<FrameStamp, Box<dyn Error>> {
        loop {
            let observed = self.wake.revision();
            self.pump()?;
            if let Some(frame) =
                prepared_frame(&self.canvas).filter(|frame| frame.content.pending())
            {
                return Ok(frame.content.target());
            }
            self.wake.wait_after(observed)?;
        }
    }

    pub(in crate::startup::canvas::tests::surface) fn commit(
        &mut self,
        id: u64,
    ) -> Result<AcceptedCamera, Box<dyn Error>> {
        self.send(id, "surface.commit", json!({}))?;
        let target = self.pending()?;
        self.canvas.presented(target)?;
        self.take(id, "applied")?;
        self.accepted()
    }

    pub(in crate::startup::canvas::tests::surface) fn take(
        &mut self,
        id: u64,
        status: &str,
    ) -> Result<Value, Box<dyn Error>> {
        let command = self.canvas.take_command().ok_or("Surface response absent")?;
        self.decode(command, id, status)
    }

    fn decode(
        &self,
        command: DesktopStartupCommand,
        id: u64,
        status: &str,
    ) -> Result<Value, Box<dyn Error>> {
        let DesktopStartupCommand::SendSurface(reply) = command else {
            return Err("Expected ordinary surface response".into());
        };
        assert_eq!(reply.identity(), self.identity);
        let value: Value = serde_json::from_slice(reply.payload())?;
        assert_eq!(value["requestId"], id);
        assert_eq!(value["status"], status);
        assert_eq!(value["session"], self.panel.as_str());
        Ok(value)
    }

    /// Advance injected display time while actual completions and native uploads release custody.
    /// Check for no-op completion before demanding a frame that need not be allocated.
    pub(in crate::startup::canvas::tests::surface) fn complete(
        &mut self,
        id: u64,
    ) -> Result<Value, Box<dyn Error>> {
        loop {
            if let Some(command) = self.canvas.take_command() {
                return self.decode(command, id, "applied");
            }
            self.advance(100);
            let observed = self.wake.revision();
            self.pump()?;
            if let Some(command) = self.canvas.take_command() {
                return self.decode(command, id, "applied");
            }
            if let Some(target) = prepared_frame(&self.canvas)
                .filter(|frame| frame.content.pending())
                .map(|frame| frame.content.target())
            {
                self.canvas.presented(target)?;
            } else {
                self.wake.wait_after(observed)?;
            }
        }
    }

    /// Retire the canvas scope and join its original execution owner under one finite cutoff.
    pub(in crate::startup::canvas::tests::surface) fn finish(self) -> Result<(), Box<dyn Error>> {
        self.finish_observing(None, None)
    }

    /// Require every resident allocation claim to retire before releasing its final observation.
    pub(in crate::startup::canvas::tests::surface) fn finish_resident(
        self,
        memory: MemoryLease,
    ) -> Result<(), Box<dyn Error>> {
        self.finish_observing(None, Some(memory))
    }

    /// Require the retained original native failure to survive scope settlement and final release.
    pub(in crate::startup::canvas::tests::surface) fn finish_failed(
        self,
        expected_code: i32,
    ) -> Result<(), Box<dyn Error>> {
        self.finish_observing(Some(expected_code), None)
    }

    fn finish_observing(
        mut self,
        expected_code: Option<i32>,
        mut resident: Option<MemoryLease>,
    ) -> Result<(), Box<dyn Error>> {
        assert!(self.canvas.take_command().is_none());
        let cutoff = self.clock.now().checked_add(Duration::from_secs(5))?;
        self.canvas.close_until(cutoff)?;
        loop {
            let observed = self.wake.revision();
            self.client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
            let settled = self.canvas.settled()?;
            if let Some(memory) = &resident
                && let Ok(claim) = memory.claim(memory.bytes())
            {
                drop(claim);
                resident = None;
                continue;
            }
            if settled {
                if resident.is_some() {
                    return Err("Closed preview retains resident allocation claims".into());
                }
                break;
            }
            self.wake.wait_after(observed).map_err(|cause| {
                format!("Canvas close progress stopped: {cause}; {:?}", self.owner.poll_shutdown())
            })?;
        }
        let canvas_result = self.canvas.finish();
        self.owner.request_shutdown(cutoff);
        let deadline =
            Instant::now().checked_add(Duration::from_secs(10)).ok_or("Deadline overflow")?;
        loop {
            self.client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
            if self.owner.poll_shutdown()?.settled {
                break;
            }
            if Instant::now() >= deadline {
                return Err("Viewport fixture execution did not settle".into());
            }
            std::hint::spin_loop();
        }
        self.owner.finish_shutdown()?;
        drop(self.memory.claim(128 * 1024)?);
        match (expected_code, canvas_result) {
            (None, result) => result,
            (Some(code), Err(cause)) => {
                assert_eq!(
                    cause.downcast_ref::<std::io::Error>().and_then(std::io::Error::raw_os_error),
                    Some(code)
                );
                Ok(())
            }
            (Some(_), Ok(())) => Err("Canvas discarded its original native failure".into()),
        }
    }
}

/// Write a flat sRGB PNG output of the given size.
pub(in crate::startup::canvas::tests::surface) fn image(
    path: &std::path::Path,
    width: u32,
    height: u32,
    color: [u8; 3],
) -> Result<(), Box<dyn Error>> {
    let file = std::fs::File::create(path)?;
    let mut encoder = png::Encoder::new(file, width, height);
    encoder.set_color(png::ColorType::Rgb);
    let mut writer = encoder.write_header()?;
    let pixels = color.repeat(usize::try_from(u64::from(width) * u64::from(height))?);
    writer.write_image_data(&pixels)?;
    writer.finish()?;
    Ok(())
}
