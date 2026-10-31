// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise placement and actual offscreen DirectComposition surface submission.

use super::*;
use crate::canvas::{CpuFrame, CpuFrameKind, placement::PhysicalRect};
use sugarsubstitute_canvas::DamagePlan;
use sugarsubstitute_contracts::execution::{Generation, SurfaceId};
use sugarsubstitute_image_ops::Extent;
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};

use crate::canvas::test_harness as harness;

mod shared;

#[test]
fn clip_uses_local_coordinates_and_rejects_exposure_outside_viewport()
-> Result<(), Box<dyn std::error::Error>> {
    let placement = CanvasPlacement {
        viewport: PhysicalRect { x: -10, y: 20, width: 100, height: 80 },
        clip: PhysicalRect { x: 0, y: 25, width: 90, height: 70 },
    };
    let actual = clip(placement)?;
    assert_eq!((actual.left, actual.top, actual.right, actual.bottom), (10.0, 5.0, 100.0, 75.0));
    assert!(
        clip(CanvasPlacement { clip: PhysicalRect { x: -11, ..placement.clip }, ..placement })
            .is_err()
    );
    assert!(
        clip(CanvasPlacement { clip: PhysicalRect { width: 91, ..placement.clip }, ..placement })
            .is_err()
    );
    assert!(
        clip(CanvasPlacement { clip: PhysicalRect { height: 0, ..placement.clip }, ..placement })
            .is_err()
    );
    Ok(())
}

/// Balance this test thread's own COM initialization without touching any visible window.
struct Apartment;
impl Apartment {
    fn enter() -> Result<Self, windows::core::Error> {
        // SAFETY: The test owns this thread and balances a successful initialization below.
        unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()? };
        Ok(Self)
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        // SAFETY: This balances the successful initialization on the same test thread.
        unsafe { CoUninitialize() };
    }
}

#[test]
fn real_offscreen_surface_commits_frames_and_preserves_old_surface_on_budget_rejection()
-> Result<(), Box<dyn std::error::Error>> {
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
        super::super::DCompositionCreateDevice2(
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
    let mut canvas = Canvas::new(composition.clone(), root, memory, graphics)?;
    let extent = Extent::new(64, 48)?;
    let pixels = vec![0xff12_3456; extent.pixels() as usize];
    let damage = DamagePlan::full(extent);
    let stamp = FrameStamp { surface: SurfaceId::new(1)?, generation: Generation::new(1)? };
    let frame = CpuFrame {
        kind: CpuFrameKind::Prepared,
        pending: true,
        target: stamp,
        base: None,
        damage: &damage,
        pixels: &pixels,
    };
    let viewport = PhysicalRect { x: 8, y: 12, width: 64, height: 48 };
    let placement = CanvasPlacement { viewport, clip: viewport };
    assert_eq!(canvas.present(&CanvasContent::Cpu(frame), placement)?, Some(stamp));
    assert!(canvas.acknowledgement.retains(stamp, placement));
    assert!(canvas.retains(stamp, placement));
    assert!(!canvas.has_retirements());
    assert!(!canvas.retains(FrameStamp { generation: Generation::new(2)?, ..stamp }, placement));
    assert!(!canvas.retains(
        stamp,
        CanvasPlacement { clip: PhysicalRect { width: 63, ..viewport }, ..placement }
    ));

    let acknowledged = CpuFrame { pending: false, ..frame };
    let moved_viewport = PhysicalRect { x: 16, ..viewport };
    let moved = CanvasPlacement { viewport: moved_viewport, clip: moved_viewport };
    assert_eq!(canvas.present(&CanvasContent::Cpu(acknowledged), moved)?, Some(stamp));
    assert_eq!(canvas.present(&CanvasContent::Cpu(acknowledged), moved)?, None);
    assert!(canvas.acknowledgement.retains(stamp, moved));

    let resized = Extent::new(80, 60)?;
    let resized_pixels = vec![0xff65_4321; resized.pixels() as usize];
    let resized_damage = DamagePlan::full(resized);
    let resized_stamp = FrameStamp { generation: Generation::new(2)?, ..stamp };
    let resized_frame = CpuFrame {
        kind: CpuFrameKind::Prepared,
        pending: true,
        target: resized_stamp,
        base: None,
        damage: &resized_damage,
        pixels: &resized_pixels,
    };
    let resized_viewport = PhysicalRect { x: 11, y: 14, width: 80, height: 60 };
    let resized_placement = CanvasPlacement {
        viewport: resized_viewport,
        clip: PhysicalRect { x: 15, y: 18, width: 72, height: 52 },
    };
    assert_eq!(
        canvas.present(&CanvasContent::Cpu(resized_frame), resized_placement)?,
        Some(resized_stamp)
    );
    assert_eq!(canvas.active.as_ref().map(Allocation::extent), Some(resized));
    assert!(canvas.present(&CanvasContent::Cpu(acknowledged), placement).is_err());
    assert!(canvas.acknowledgement.retains(resized_stamp, resized_placement));

    let probe_bytes = 16 * 1024 * 1024 - 8 * 1024 * 1024 - (80 * 60 * 12 + 1024 * 1024);
    assert!(canvas.lease.as_ref().ok_or("Canvas lease absent")?.claim(probe_bytes).is_err());
    assert!(!canvas.hide()?);
    assert!(canvas.active.is_none());
    assert_eq!(canvas.retirements.iter().flatten().count(), 2);
    assert!(canvas.release_after_detach().is_err());
    let first = canvas.retirements[0].as_ref().ok_or("Old surface absent")?.commit.fence.clone();
    assert!(!first.completed());
    execution
        .wait_composition(canvas.next_wait().ok_or("First wait absent")?)
        .map_err(|cause| cause as Box<dyn std::error::Error>)?;
    assert!(first.completed());
    assert!(canvas.lease.as_ref().ok_or("Canvas lease absent")?.claim(probe_bytes).is_err());
    canvas.reap_retirements()?;
    assert_eq!(canvas.retirements.iter().flatten().count(), 1);
    drop(canvas.lease.as_ref().ok_or("Canvas lease absent")?.claim(probe_bytes)?);
    assert_eq!(canvas.present(&CanvasContent::Cpu(acknowledged), placement)?, None);
    execution
        .wait_composition(canvas.next_wait().ok_or("Hide wait absent")?)
        .map_err(|cause| cause as Box<dyn std::error::Error>)?;
    canvas.reap_retirements()?;
    assert!(!canvas.has_retirements());
    assert!(canvas.hide()?);
    assert_eq!(canvas.present(&CanvasContent::Cpu(acknowledged), placement)?, Some(stamp));

    let large = Extent::new(1024, 1024)?;
    let large_pixels = vec![0xffab_cdef; large.pixels() as usize];
    let large_damage = DamagePlan::full(large);
    let next = FrameStamp { generation: Generation::new(3)?, ..stamp };
    let large_frame = CpuFrame {
        kind: CpuFrameKind::Prepared,
        pending: true,
        target: next,
        base: None,
        damage: &large_damage,
        pixels: &large_pixels,
    };
    let large_viewport = PhysicalRect { x: 8, y: 12, width: 1024, height: 1024 };
    assert!(
        canvas
            .present(
                &CanvasContent::Cpu(large_frame),
                CanvasPlacement { viewport: large_viewport, clip: large_viewport }
            )
            .is_err()
    );
    assert!(canvas.acknowledgement.retains(stamp, placement));
    assert_eq!(canvas.active.as_ref().map(Allocation::extent), Some(extent));
    assert!(canvas.pending.is_none());
    assert!(!canvas.hide()?);
    assert!(!canvas.acknowledgement.retains(stamp, placement));
    assert!(!canvas.retains(stamp, placement));
    assert!(canvas.active.is_none());
    assert!(canvas.has_retirements());
    assert!(!canvas.hide()?);
    execution
        .wait_composition(canvas.next_wait().ok_or("Final hide wait absent")?)
        .map_err(|cause| cause as Box<dyn std::error::Error>)?;
    canvas.reap_retirements()?;
    assert!(canvas.hide()?);
    assert_eq!(canvas.present(&CanvasContent::Cpu(acknowledged), placement)?, Some(stamp));
    canvas.end_draws()?;
    // This root was never bound to a target, so no native window can retain the visual tree.
    canvas.release_after_detach()?;
    drop(canvas);
    execution.finish().map_err(|cause| cause as Box<dyn std::error::Error>)?;
    Ok(())
}
