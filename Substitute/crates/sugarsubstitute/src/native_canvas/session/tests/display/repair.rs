// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! A CPU repair mutates the canonical backing only while this session owns it exclusively.

use super::*;
use sugarsubstitute_canvas::DamagePlan;
use sugarsubstitute_contracts::execution::JobPhase;

/// Prepare one complete repair of the retained fallback backing on the CPU lane.
struct RepairBoundary {
    base: FrameStamp,
    target: FrameStamp,
    view: Viewport,
    document: work::DocumentKey,
}
impl CpuTask<work::Packet, WorkError> for RepairBoundary {
    fn step(&mut self, context: &JobContext) -> Result<CpuStep<work::Packet>, WorkError> {
        let update = PreparedUpdate::prepare(
            self.target,
            Some(self.base),
            DamagePlan::full(self.view.surface()),
            context.memory(),
            |_, pixels| {
                pixels.fill(0xff20_4060);
                Ok(())
            },
        )?;
        Ok(CpuStep::Complete(work::Packet {
            sampling_scale: self.view.zoom(),
            tier: BackingTier::Fallback,
            reconstruction: Reconstruction::Preview,
            display_buffers: None,
            pixels: work::Pixels::Cpu(work::CpuPacket::Repair(Box::new(update))),
            view: self.view,
            document: self.document,
            elapsed: Duration::ZERO,
            stats: sugarsubstitute_image_ops::EvaluationStats::default(),
            source_revision: 7,
            source_io: None,
        }))
    }
}

/// A current CPU document whose fallback repair has already reached its terminal outcome.
fn completed_repair() -> Result<(Fixture, FrameStamp), Box<dyn Error>> {
    let (mut session, clock, directory) = session()?;
    let wake = Arc::new(Wake::default());
    let source = FrameStamp { surface: SurfaceId::new(1)?, generation: Generation::new(1)? };
    let repaired = FrameStamp { generation: Generation::new(2)?, ..source };
    let view = session.document.tools.navigation().viewport();
    let products = cpu_display_products(
        &mut session,
        wake.clone(),
        CpuDisplayBoundary {
            source,
            target: FrameStamp { generation: Generation::new(3)?, ..source },
            second_target: FrameStamp { generation: Generation::new(4)?, ..source },
            source_view: view,
            projected_view: view,
            replacement_extent: view.surface(),
        },
    )?;
    let runtime = session.runtime.as_ref().ok_or("Repair fixture execution is unavailable")?;
    let setup_spec = session.spec(
        Operation::Infrastructure,
        JobClass::InfrastructureOrdinary,
        65536,
        Duration::from_secs(30),
        None,
    )?;
    let mut setup = runtime.client.submit_blocking(&runtime.scope, setup_spec, |context| {
        Document::new(1, Extent::new(640, 480)?, context.memory())
    })?;
    let JobOutcome::Success(document) = wake.completion(&runtime.client, &mut setup)?.outcome
    else {
        return Err("Repair fixture document failed".into());
    };
    let snapshot = document.snapshot();
    let identity = (snapshot.identity(), snapshot.revision(), 0);
    let spec = session.spec(
        Operation::FramePrepare,
        JobClass::CpuInteractive,
        4 * 1024 * 1024,
        Duration::from_secs(30),
        Some(ReplacementKey { surface: SurfaceId::new(1)?, revision: repaired.generation }),
    )?;
    let coverage = BackingCoverage::new(view, [0; 2])?;
    let boundary = RepairBoundary {
        base: source,
        target: repaired,
        view: coverage.viewport(),
        document: identity,
    };
    let job = runtime.client.submit_cpu(&runtime.scope, spec, move |_| boundary)?;
    while !matches!(job.status().phase, JobPhase::Terminal(_)) {
        let observed = wake.revision();
        runtime.client.drain_completions(std::num::NonZeroUsize::MIN)?;
        if !matches!(job.status().phase, JobPhase::Terminal(_)) {
            wake.wait_after(observed);
        }
    }
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
    session.document.fallback_front = Some(products.surface);
    session.rendering = Some(Rendering {
        comparison: None,
        job,
        document: identity,
        snapshot,
        source_revision: 7,
        reconstruction: Reconstruction::Preview,
        tier: BackingTier::Fallback,
        coverage,
        display_extent: view.surface(),
        superseded: false,
    });
    Ok((Fixture { session, clock, wake, view, source, _directory: directory }, repaired))
}

#[test]
fn a_repair_waits_for_display_work_to_release_the_backing_after_presentation_is_revoked()
-> Result<(), Box<dyn Error>> {
    let (mut fixture, repaired) = completed_repair()?;
    // Revoked presentation leaves its cancelled display job running with the backing clone.
    fixture.session.drawable = false;
    let display_clone = fixture.session.document.fallback_front.clone();

    fixture.session.consume_frame()?;
    assert!(fixture.session.rendering.is_some(), "repair was consumed while borrowed");
    assert!(fixture.session.failed.is_none());
    let front = fixture.session.document.fallback_front.as_ref().ok_or("Backing was lost")?;
    assert_eq!(front.current(), Some(fixture.source));
    assert!(fixture.session.document.fallback_presented.is_none());

    // The display job's terminal follows the release of its clone and wakes this owner.
    drop(display_clone);
    fixture.session.consume_frame()?;
    assert!(fixture.session.rendering.is_none());
    assert!(fixture.session.failed.is_none());
    let front = fixture.session.document.fallback_front.as_ref().ok_or("Backing was lost")?;
    assert_eq!(front.current(), Some(repaired));
    assert!(fixture.session.document.fallback_presented.is_some());
    Ok(())
}
