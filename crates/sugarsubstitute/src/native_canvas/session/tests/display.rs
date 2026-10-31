// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Application publication labels actual completed display poses and rejects obsolete content.
mod activity;
mod composite;
mod presentation;
mod source_fencing;
mod surface;
use super::refinement::Wake;
use super::*;
use sugarsubstitute_canvas::{BackingCoverage, Document, ViewProjection};
use sugarsubstitute_canvas_render::{PreparedUpdate, ProjectionBuffer};
use sugarsubstitute_desktop::canvas::CpuFrameKind;
use sugarsubstitute_image_ops::Pixel;

/// Real canonical pixels and paired projection allocations created under one admitted job.
pub(super) struct CpuDisplayProducts {
    pub(super) document: Document,
    pub(super) surface: Arc<CpuSurface>,
    pub(super) projection: Box<CpuProjection>,
    pub(super) second_projection: Box<CpuProjection>,
    pub(super) spare: ProjectionBuffer,
    pub(super) replacement: [Option<ProjectionBuffer>; 2],
}
/// Create real application display products through the execution-owned CPU task boundary.
pub(super) struct CpuDisplayBoundary {
    pub(super) source: FrameStamp,
    pub(super) target: FrameStamp,
    pub(super) second_target: FrameStamp,
    pub(super) source_view: Viewport,
    pub(super) projected_view: Viewport,
    pub(super) replacement_extent: Extent,
}
impl CpuTask<CpuDisplayProducts, WorkError> for CpuDisplayBoundary {
    fn step(&mut self, context: &JobContext) -> Result<CpuStep<CpuDisplayProducts>, WorkError> {
        let damage = DamagePlan::full(self.source_view.surface());
        let document = Document::new(1, Extent::new(1, 1)?, context.memory())?;
        let prepared =
            PreparedUpdate::prepare(self.source, None, damage, context.memory(), |_, pixels| {
                pixels.fill(0xff4269a5);
                Ok(())
            })?;
        let surface = Arc::new(CpuSurface::from_prepared(prepared)?);
        let mapping = ViewProjection::new(self.source_view, self.projected_view)?;
        let projection =
            ProjectionBuffer::new(self.projected_view.surface(), context.memory(), context)?
                .project(
                    self.target,
                    &surface,
                    mapping,
                    Pixel::linear(0.018, 0.018, 0.018, 1.0)?,
                    context,
                )?;
        let spare =
            ProjectionBuffer::new(self.projected_view.surface(), context.memory(), context)?;
        let second_projection =
            ProjectionBuffer::new(self.projected_view.surface(), context.memory(), context)?
                .project(
                    self.second_target,
                    &surface,
                    mapping,
                    Pixel::linear(0.018, 0.018, 0.018, 1.0)?,
                    context,
                )?;
        let replacement = [
            Some(ProjectionBuffer::new(self.replacement_extent, context.memory(), context)?),
            Some(ProjectionBuffer::new(self.replacement_extent, context.memory(), context)?),
        ];
        Ok(CpuStep::Complete(CpuDisplayProducts {
            document,
            surface,
            projection: Box::new(projection),
            second_projection: Box::new(second_projection),
            spare,
            replacement,
        }))
    }
}
/// A real admitted document and one typed completed display request, without a native GPU device.
struct Fixture {
    session: Session,
    clock: Arc<TestClock>,
    wake: Arc<Wake>,
    view: Viewport,
    source: FrameStamp,
    _directory: tempfile::TempDir,
}
/// Construct original document custody on a worker before faking only the native draw result.
fn pending_draw() -> Result<Fixture, Box<dyn Error>> {
    pending_draw_outcome(false, false)
}
/// Deliver failure or a forged source identity through the real owned completion boundary.
fn pending_draw_outcome(failure: bool, foreign_payload: bool) -> Result<Fixture, Box<dyn Error>> {
    let (mut session, clock, directory) = session()?;
    session.config.gpu =
        Some(crate::native_canvas::gpu_policy::GpuRequest::Required("vulkan:8086:a780".parse()?));
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(session.clock.clone(), wake.clone())?;
    let client = owner.client();
    let runtime =
        Runtime::owned(owner, client.clone(), ScopeSpec { id: ScopeId::new(1)?, deadline: None })?;
    let spec = session.spec(
        Operation::Infrastructure,
        JobClass::InfrastructureOrdinary,
        65536,
        Duration::from_secs(30),
        None,
    )?;
    let mut setup = client.submit_blocking(&runtime.scope, spec, |context| {
        Document::new(1, Extent::new(640, 480)?, context.memory())
    })?;
    let JobOutcome::Success(document) = wake.completion(&client, &mut setup)?.outcome else {
        return Err("Draw fixture document failed".into());
    };
    let identity = (document.snapshot().identity(), document.snapshot().revision());
    session.document.scene = Some(Scene {
        document,
        source: None,
        source_revision: 7,
        original: None,
        compared: None,
        placed: Vec::new(),
    });
    session.selection = 7;
    session.source_sequence = 7;
    let view = session.document.tools.navigation().viewport();
    let source = FrameStamp { surface: SurfaceId::new(1)?, generation: Generation::new(1)? };
    let target = FrameStamp { generation: Generation::new(2)?, ..source };
    let coverage = BackingCoverage::new(view, [0; 2])?;
    let projection = ViewProjection::new(view, view)?;
    let spec = session.display_spec(target, true)?;
    let mut drawing = Some(work::Draw {
        underlay: None,
        tier: BackingTier::Detail,
        source,
        target,
        view,
        coverage,
        projection,
        pixels: work::DrawPixels::Gpu(None),
        document: if foreign_payload { (identity.0 + 1, identity.1) } else { identity },
        source_revision: 7,
        verified: true,
    });
    let job = client.submit_blocking_owned_input(
        &runtime.scope,
        spec,
        &mut drawing,
        move |drawing, _| {
            if failure { Err(WorkError::invalid("Display boundary failure")) } else { Ok(drawing) }
        },
    )?;
    session.pacing.submitted(
        target,
        PresentationMode::Canonical,
        Duration::ZERO,
        Duration::ZERO,
    )?;
    session.displaying = Some(super::super::display::Displaying {
        target,
        document: identity,
        source_revision: 7,
        job,
    });
    session.runtime = Some(runtime);
    Ok(Fixture { session, clock, wake, view, source, _directory: directory })
}
/// Observe the typed completion through its real application consumer before asserting publication.
fn consume(session: &mut Session, wake: &Wake) -> Result<(), Box<dyn Error>> {
    while session.displaying.is_some() {
        let observed = wake.revision();
        session
            .runtime
            .as_ref()
            .ok_or("Draw fixture execution absent")?
            .client
            .drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
        session.consume_display()?;
        if session.displaying.is_some() {
            wake.wait_after(observed)?;
        }
    }
    Ok(())
}

/// Admit the exact source and paired display allocations retained by the test session.
pub(super) fn cpu_display_products(
    session: &mut Session,
    wake: Arc<Wake>,
    boundary: CpuDisplayBoundary,
) -> Result<CpuDisplayProducts, Box<dyn Error>> {
    let owner = ExecutionOwner::new(session.clock.clone(), wake.clone())?;
    let client = owner.client();
    let runtime =
        Runtime::owned(owner, client.clone(), ScopeSpec { id: ScopeId::new(1)?, deadline: None })?;
    let spec = session.spec(
        Operation::FramePrepare,
        JobClass::CpuInteractive,
        4 * 1024 * 1024,
        Duration::from_secs(30),
        Some(ReplacementKey { surface: SurfaceId::new(2)?, revision: boundary.target.generation }),
    )?;
    let mut job = client.submit_cpu(&runtime.scope, spec, move |_| boundary)?;
    let JobOutcome::Success(products) = wake.completion(&client, &mut job)?.outcome else {
        return Err("CPU display fixture failed".into());
    };
    session.runtime = Some(runtime);
    Ok(products)
}

#[test]
fn acknowledged_cpu_projection_publishes_actual_pose_once() -> Result<(), Box<dyn Error>> {
    let (mut session, _clock, _directory) = session()?;
    let wake = Arc::new(Wake::default());
    let source = FrameStamp { surface: SurfaceId::new(1)?, generation: Generation::new(1)? };
    let target = FrameStamp { generation: Generation::new(2)?, ..source };
    let foreign = FrameStamp { generation: Generation::new(3)?, ..source };
    let second_target = FrameStamp { generation: Generation::new(4)?, ..source };
    let source_view = session.document.tools.navigation().viewport();
    let mut projected_view = source_view;
    projected_view.pan_by([11.0, -7.0])?;
    let mut latest_view = projected_view;
    latest_view.pan_by([-3.0, 5.0])?;
    session.document.tools = ToolManager::new(Navigation::new(latest_view));
    let replacement_extent = Extent::new(400, 300)?;

    let products = cpu_display_products(
        &mut session,
        wake,
        CpuDisplayBoundary {
            source,
            target,
            second_target,
            source_view,
            projected_view,
            replacement_extent,
        },
    )?;
    let document_identity =
        (products.document.snapshot().identity(), products.document.snapshot().revision());
    session.document.scene = Some(Scene {
        document: products.document,
        source: None,
        source_revision: 0,
        original: None,
        compared: None,
        placed: Vec::new(),
    });
    let source_revision = 0;
    session.document.front = Some(products.surface);
    session.document.cpu_projection = Some(products.projection);
    session.document.display_buffers = [Some(products.spare), None];
    session.document.display_buffer_extent = Some(projected_view.surface());
    session.document.projection_damage = Some(DamagePlan::full(projected_view.surface()));
    session.document.cpu_pending = Some(target);
    let coverage = BackingCoverage::new(source_view, [0; 2])?;
    session.document.pending_cpu_evidence = Some(DisplayEvidence {
        underlay: None,
        tier: BackingTier::Fallback,
        target,
        source,
        view: projected_view,
        coverage,
        projection: ViewProjection::new(source_view, projected_view)?,
        document: document_identity,
        source_revision,
    });
    session.pacing.submitted(
        target,
        PresentationMode::Projected,
        Duration::ZERO,
        Duration::ZERO,
    )?;
    session.pacing.prepared(target, Duration::ZERO)?;

    assert!(session.needs_display_buffers(replacement_extent));
    let retired = session.install_display_buffers(products.replacement, replacement_extent);
    assert!(retired[0].is_some());
    assert!(!session.needs_display_buffers(replacement_extent));
    assert_eq!(
        session.frame().ok_or("Old acknowledged frame disappeared during pair replacement")?.target,
        target
    );

    let pending = session.frame().ok_or("CPU projection was not published")?;
    assert_eq!(pending.kind, CpuFrameKind::Projected);
    assert!(pending.pending);
    assert_eq!(pending.target, target);
    assert!(session.did_present_cpu(foreign).is_err());
    assert_eq!(session.document.cpu_pending, Some(target));
    assert_eq!(session.fallback_projection_acceptances, 0);
    assert!(matches!(
        session.pacing.readiness(Duration::ZERO)?,
        PresentationReadiness::Busy { target: retained, .. } if retained == target
    ));

    session.did_present_cpu(target)?;
    assert_eq!(session.document.cpu_pending, None);
    assert_eq!(session.document.display_source, Some(source));
    assert_eq!(session.document.display_view, Some(projected_view));
    assert_ne!(session.document.display_view, Some(latest_view));
    assert_eq!(session.document.native_presented, Some(target));
    let evidence = session.document.display_evidence.ok_or("Accepted display evidence was lost")?;
    assert_eq!(evidence.target, target);
    assert_eq!(evidence.tier, BackingTier::Fallback);
    assert_eq!(evidence.source, source);
    assert_eq!(evidence.view, projected_view);
    assert_eq!(evidence.projection.source(), source_view);
    assert_eq!(evidence.projection.target(), projected_view);
    assert_eq!(session.fallback_projection_acceptances, 1);
    assert!(!session.frame().ok_or("Acknowledged projection was lost")?.pending);
    assert_eq!(session.pacing.stats().projected_accepted, 1);
    assert!(session.did_present_cpu(target).is_err());

    Ok(())
}

#[test]
fn completed_display_keeps_actual_camera_while_latest_zoom_intent_skips_missed_ticks()
-> Result<(), Box<dyn Error>> {
    let Fixture { mut session, clock, wake, view, source, _directory } = pending_draw()?;
    session.input(Input::Tool(ToolInput::Wheel { steps: 1.0, phase: ScrollPhase::Unphased }))?;
    clock.0.store(250_000_000, Ordering::SeqCst);
    session.document.tools.advance(Duration::from_millis(250))?;
    let latest = session.document.tools.navigation().viewport();
    assert_ne!(latest, view);
    consume(&mut session, &wake)?;
    assert_eq!(session.document.display_source, Some(source));
    assert_eq!(session.document.display_view, Some(view));
    assert_eq!(session.document.tools.navigation().viewport(), latest);
    assert_eq!(session.document.tools.navigation().target_viewport(), latest);
    assert!(!session.document.tools.navigation().interacting());
    assert!(session.failed.is_none());
    Ok(())
}

#[test]
fn obsolete_content_and_resized_native_outputs_retain_custody_for_worker_retirement()
-> Result<(), Box<dyn Error>> {
    for resized in [false, true] {
        let Fixture { mut session, wake, _directory, .. } = pending_draw()?;
        if resized {
            session.resize(Extent::new(400, 300)?)?;
        } else {
            session.document.scene.as_mut().ok_or("Draw fixture scene absent")?.source_revision +=
                1;
        }
        consume(&mut session, &wake)?;
        assert_eq!(session.document.display_source, None);
        assert_eq!(session.document.display_view, None);
        assert!(
            session.pending_retirement.as_ref().is_some_and(|retired| retired._drawing.is_some())
        );
        assert!(session.failed.is_none());
    }
    Ok(())
}
