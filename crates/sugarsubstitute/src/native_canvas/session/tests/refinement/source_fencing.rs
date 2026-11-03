// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep observed preparation failures bound to their originating authored source.
use super::*;
use sugarsubstitute_canvas::{DocumentEdit, Layer, LayerContent, LayerId};
use sugarsubstitute_canvas_render::PreparedUpdate;

#[test]
fn replaced_source_render_failures_do_not_fail_the_selected_document() -> Result<(), Box<dyn Error>>
{
    for superseded in [false, true] {
        let Fixture { mut session, wake, boundary, _directory } =
            preparing(Reconstruction::Finished(sugarsubstitute_image_ops::Filter::Lanczos3))?;
        session.rendering.as_mut().ok_or("Missing old preparation")?.superseded = superseded;
        session.document.scene.as_mut().ok_or("Missing selected scene")?.source_revision = 1;
        session.selection = 1;
        session.source_sequence = 1;
        let title = session.title.clone();
        boundary.fail();
        observe_terminal(&mut session, &wake, &boundary)?;
        assert!(session.failed.is_none());
        assert_eq!(session.title, title);
        assert!(session.rendering.is_none());
    }
    Ok(())
}

#[test]
fn current_source_render_failures_preserve_superseded_failure_behavior()
-> Result<(), Box<dyn Error>> {
    for superseded in [false, true] {
        let Fixture { mut session, wake, boundary, _directory } =
            preparing(Reconstruction::Finished(sugarsubstitute_image_ops::Filter::Lanczos3))?;
        session.rendering.as_mut().ok_or("Missing current preparation")?.superseded = superseded;
        boundary.fail();
        observe_terminal(&mut session, &wake, &boundary)?;
        assert_eq!(
            session.failed.as_ref().ok_or("Current preparation failure was lost")?.to_string(),
            "Preparation boundary failure"
        );
    }
    Ok(())
}

#[test]
fn changed_authored_revision_retires_render_failure_for_the_same_raster_source()
-> Result<(), Box<dyn Error>> {
    let Fixture { mut session, wake, boundary, _directory } =
        preparing(Reconstruction::Finished(sugarsubstitute_image_ops::Filter::Lanczos3))?;
    session.document.scene.as_mut().ok_or("Missing selected scene")?.document.apply(
        DocumentEdit::Insert {
            index: 0,
            layer: Layer::new(LayerId::new(1)?, LayerContent::Exposure { stops: 1.0 })?,
        },
    )?;
    boundary.fail();
    observe_terminal(&mut session, &wake, &boundary)?;
    assert!(session.failed.is_none());
    Ok(())
}

#[test]
fn successful_render_rejects_payload_outside_its_admitted_document_or_source()
-> Result<(), Box<dyn Error>> {
    for foreign_document in [false, true] {
        let Fixture { mut session, wake, boundary, _directory } =
            preparing(Reconstruction::Preview)?;
        let rendering = session.rendering.as_mut().ok_or("Missing preparation")?;
        rendering.superseded = true;
        rendering.job.cancel();
        observe_terminal(&mut session, &wake, &boundary)?;
        let snapshot =
            session.document.scene.as_ref().ok_or("Missing selected scene")?.document.snapshot();
        let document = (snapshot.identity(), snapshot.revision());
        let view = Viewport::new(Extent::new(1, 1)?)?;
        let coverage = BackingCoverage::new(view, [0; 2])?;
        let target = FrameStamp { surface: SurfaceId::new(1)?, generation: Generation::new(2)? };
        let spec = session.spec(
            Operation::Infrastructure,
            JobClass::InfrastructureOrdinary,
            65536,
            Duration::from_secs(30),
            None,
        )?;
        let runtime = session.runtime.as_ref().ok_or("Missing execution")?;
        let job = runtime.client.submit_blocking(&runtime.scope, spec, move |context| {
            let update = PreparedUpdate::prepare(
                target,
                None,
                DamagePlan::full(view.surface()),
                context.memory(),
                |_, output| {
                    output.fill(0xff42_69a5);
                    Ok(())
                },
            )?;
            Ok::<_, WorkError>(Packet {
                sampling_scale: view.zoom(),
                tier: BackingTier::Detail,
                reconstruction: Reconstruction::Preview,
                display_buffers: None,
                pixels: work::Pixels::Cpu(work::CpuPacket::Replace {
                    surface: CpuSurface::from_prepared(update)?,
                }),
                view,
                document: if foreign_document { (document.0 + 1, document.1) } else { document },
                elapsed: Duration::ZERO,
                stats: Default::default(),
                source_revision: u64::from(!foreign_document),
                source_io: None,
            })
        })?;
        session.rendering = Some(Rendering {
            comparison: None,
            job,
            document,
            snapshot,
            source_revision: 0,
            reconstruction: Reconstruction::Preview,
            tier: BackingTier::Detail,
            coverage,
            display_extent: view.surface(),
            superseded: false,
        });
        assert!(observe_terminal(&mut session, &wake, &boundary).is_err());
        assert!(session.document.front.is_none());
        assert!(
            session.pending_retirement.as_ref().is_some_and(|retired| retired._packet.is_some())
        );
    }
    Ok(())
}
