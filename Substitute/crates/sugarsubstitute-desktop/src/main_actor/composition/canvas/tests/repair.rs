// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! A CPU frame whose plan is relative to the frame the surface retains is shown by repairing the
//! surface; every other frame redraws it whole. Each repaired region is read back exactly.

use super::{
    super::timing::{Reach, Source, UploadCost},
    *,
};
use sugarsubstitute_canvas::{DirtyRegions, Viewport};
use sugarsubstitute_image_ops::Region;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const WIDTH: u32 = 96;
const HEIGHT: u32 = 64;

fn stamp(generation: u64) -> TestResult<FrameStamp> {
    Ok(FrameStamp { surface: SurfaceId::new(4)?, generation: Generation::new(generation)? })
}

/// A plan that keeps everything but the device pixels one document change reaches.
fn local(extent: Extent, change: Region) -> TestResult<DamagePlan> {
    let mut changed = DirtyRegions::empty(extent);
    changed.insert(change);
    let mut plan = DamagePlan::unchanged(extent);
    plan.include_document_damage(&changed, Viewport::new(extent)?)?;
    Ok(plan)
}

/// The pixels of `region` out of a complete frame, row by row.
fn cut(pixels: &[u32], region: Region) -> Vec<u32> {
    (region.y()..region.bottom())
        .flat_map(|row| {
            let start = (row * WIDTH + region.x()) as usize;
            pixels[start..start + region.width() as usize].iter().copied()
        })
        .collect()
}

struct Shown {
    cost: UploadCost,
    /// The first region the frame drew, read back from the surface update.
    captured: Vec<u32>,
}

fn present(
    canvas: &mut Canvas,
    generation: u64,
    base: Option<u64>,
    plan: &DamagePlan,
    pixels: &[u32],
) -> TestResult<Shown> {
    let viewport = PhysicalRect { x: 0, y: 0, width: WIDTH, height: HEIGHT };
    let frame = CpuFrame {
        kind: CpuFrameKind::Prepared,
        pending: true,
        target: stamp(generation)?,
        base: base.map(stamp).transpose()?,
        damage: plan,
        pixels,
    };
    canvas.capture_next = true;
    if let Some(active) = canvas.active.as_mut() {
        // Whatever the update buffer last held must never show through a repair.
        active.capture.leave_stale(0xffff_00ff);
    }
    assert_eq!(
        canvas.present(&CanvasContent::Cpu(frame), CanvasPlacement { viewport, clip: viewport })?,
        Some(frame.target)
    );
    let cost = canvas.timings.latest.ok_or("Presentation cost absent")?;
    let captured = canvas
        .active
        .as_mut()
        .and_then(|allocation| allocation.capture.take())
        .unwrap_or_default();
    Ok(Shown { cost, captured })
}

#[test]
fn a_frame_based_on_the_retained_frame_repairs_only_what_changed() -> TestResult {
    let _apartment = Apartment::enter()?;
    let ((memory, graphics), mut execution) = harness::run_blocking(16 * 1024 * 1024, |context| {
        let memory = context.memory().clone();
        let graphics = crate::canvas::CanvasGraphics::prepare(&memory, &context, None)?;
        Ok::<_, crate::canvas::CanvasGraphicsError>((memory, graphics))
    })
    .map_err(|cause| cause as Box<dyn std::error::Error>)?;
    let mut raw = std::ptr::null_mut();
    // SAFETY: The requested device IID and writable output slot belong to this test apartment.
    let status = unsafe {
        super::super::super::DCompositionCreateDevice2(
            std::ptr::null_mut(),
            &IDCompositionDevice::IID,
            &mut raw,
        )
    };
    let mut composition: Option<IDCompositionDevice> = None;
    // SAFETY: This adopts the exact transferred device reference, including partial failure.
    unsafe { adopt(&mut composition, raw, status, "test_canvas_device")? };
    let composition = composition.ok_or("Composition device missing")?;
    // SAFETY: This test retains the device and its unbound root on the creating thread.
    let root = unsafe { composition.CreateVisual()? };
    let mut canvas = Canvas::new(composition.clone(), root, memory, graphics, true)?;
    let extent = Extent::new(WIDTH, HEIGHT)?;
    let complete = extent.pixels() * 4;
    let full = DamagePlan::full(extent);
    // Every pixel differs from its neighbours and from the same pixel of every other frame, so a
    // region drawn from the wrong place or the wrong frame cannot read back as expected.
    let frame = |seed: u32| -> Vec<u32> {
        (0..extent.pixels() as u32).map(|index| 0xff00_0000 | (seed << 16) | index).collect()
    };

    // The first frame has nothing to be relative to: the whole surface, from a new bitmap.
    let first = present(&mut canvas, 1, None, &full, &frame(1))?;
    assert_eq!((first.cost.reach, first.cost.source), (Reach::Whole, Source::Created));
    assert_eq!(first.cost.source_bytes, complete);
    assert_eq!(first.captured, frame(1));

    // A small change relative to that frame: one region is copied and one region is drawn.
    let edit = local(extent, Region::new(40, 24, 8, 8)?)?;
    let edited = edit.dirty().regions().next().ok_or("Edit reaches no pixel")?;
    assert!(edited.pixels() < extent.pixels() / 4);
    let second = present(&mut canvas, 2, Some(1), &edit, &frame(2))?;
    assert_eq!(second.cost.reach, Reach::Repaired { scrolled: false, regions: 1 });
    assert_eq!((second.cost.source, second.cost.source_bytes), (Source::Updated, edited.pixels() * 4));
    assert_eq!(second.captured, cut(&frame(2), edited));

    // A frame that changes no pixel draws and copies nothing; the retained surface is the frame.
    let same = present(&mut canvas, 3, Some(2), &DamagePlan::unchanged(extent), &frame(2))?;
    assert_eq!(same.cost.reach, Reach::Repaired { scrolled: false, regions: 0 });
    assert_eq!(same.cost.source_bytes, 0);
    assert!(same.captured.is_empty());

    // A plan relative to a frame the surface no longer holds (a skipped generation) cannot be
    // repaired: everything is copied and drawn.
    let skipped = present(&mut canvas, 4, Some(2), &edit, &frame(4))?;
    assert_eq!((skipped.cost.reach, skipped.cost.source), (Reach::Whole, Source::Updated));
    assert_eq!(skipped.cost.source_bytes, complete);
    assert_eq!(skipped.captured, frame(4));

    // A pan moves the retained pixels on the surface and draws only the exposed strip.
    let pan = DamagePlan::pan(extent, 8, 0)?;
    let strip = pan.dirty().regions().next().ok_or("Pan exposes nothing")?;
    assert_eq!((strip.width(), strip.height()), (8, HEIGHT));
    let panned = present(&mut canvas, 5, Some(4), &pan, &frame(5))?;
    assert_eq!(panned.cost.reach, Reach::Repaired { scrolled: true, regions: 1 });
    assert_eq!(panned.cost.source_bytes, strip.pixels() * 4);
    assert_eq!(panned.captured, cut(&frame(5), strip));
    // A diagonal pan exposes two disjoint strips, each drawn in its own update.
    let diagonal = DamagePlan::pan(extent, -6, 5)?;
    let exposed: Vec<Region> = diagonal.dirty().regions().collect();
    assert_eq!(exposed.len(), 2);
    let slid = present(&mut canvas, 6, Some(5), &diagonal, &frame(6))?;
    assert_eq!(slid.cost.reach, Reach::Repaired { scrolled: true, regions: 2 });
    assert_eq!(slid.cost.source_bytes, exposed.iter().map(|region| region.pixels() * 4).sum::<u64>());
    assert_eq!(slid.captured, cut(&frame(6), exposed[0]));

    // The pans moved pixels on the surface only, so the bitmap no longer holds a whole frame.
    // A repair still draws its own region exactly, transparent pixels over the wash and never
    // over what the update buffer held.
    let mut matte = frame(7);
    for pixel in &mut matte {
        if *pixel % 3 == 0 {
            *pixel = 0;
        }
    }
    let seventh = present(&mut canvas, 7, Some(6), &edit, &matte)?;
    assert_eq!(seventh.cost.reach, Reach::Repaired { scrolled: false, regions: 1 });
    assert_eq!(seventh.cost.source_bytes, edited.pixels() * 4);
    let washed: Vec<u32> = cut(&matte, edited).into_iter().map(over_wash).collect();
    assert!(
        seventh.captured.iter().zip(&washed).all(|(shown, expected)| close(*shown, *expected)),
        "{:08x?} against {washed:08x?}",
        seventh.captured
    );
    // And a whole frame after it is copied completely, never patched onto the stale bitmap.
    let whole = present(&mut canvas, 8, None, &full, &frame(8))?;
    assert_eq!((whole.cost.reach, whole.cost.source_bytes), (Reach::Whole, complete));
    assert_eq!(whole.captured, frame(8));

    // With the bitmap whole again, a repair that also redraws the whole surface later needs only
    // the changed region copied: the bitmap followed the frames in between.
    let ninth = present(&mut canvas, 9, Some(8), &edit, &frame(9))?;
    assert_eq!(ninth.cost.source_bytes, edited.pixels() * 4);
    assert!(canvas.active.as_ref().is_some_and(|active| active.holds() == (stamp(9).ok(), stamp(9).ok())));

    assert!(!canvas.hide()?);
    execution
        .wait_composition(canvas.next_wait().ok_or("Hide wait absent")?)
        .map_err(|cause| cause as Box<dyn std::error::Error>)?;
    canvas.reap_retirements()?;
    canvas.end_draws()?;
    canvas.release_after_detach()?;
    drop(canvas);
    execution.finish().map_err(|cause| cause as Box<dyn std::error::Error>)?;
    Ok(())
}
