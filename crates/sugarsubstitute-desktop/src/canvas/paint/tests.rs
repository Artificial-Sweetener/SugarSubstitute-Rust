// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify real GDI crop origins, overlapping blits and exposure against independent source pixels.
use super::*;
use crate::native_storage::Dib;
use ::windows::Win32::Graphics::Gdi::{GdiFlush, IntersectClipRect, SelectClipRgn};
use std::num::NonZeroUsize;
use sugarsubstitute_contracts::{
    execution::{Generation, SurfaceId},
    presentation::{FrameIdentity, FrameShape, FrameValidator, PreparedFrame},
};
use sugarsubstitute_image_ops::ImageError;
mod harness;

/// Real admission produces the original source/native allocation lease; no test mints a budget.
fn admitted() -> Result<PreparedFrame, Box<dyn Error>> {
    harness::run(1024 * 1024, |context| {
        let shape =
            FrameShape::new(31, 17, 1.0).map_err(|cause| ImageError::Source(Box::new(cause)))?;
        let identity = FrameIdentity {
            surface: SurfaceId::new(1).map_err(|cause| ImageError::Source(Box::new(cause)))?,
            state_generation: Generation::new(1)
                .map_err(|cause| ImageError::Source(Box::new(cause)))?,
            sequence: Generation::new(1).map_err(|cause| ImageError::Source(Box::new(cause)))?,
        };
        let mut validator = FrameValidator::new(
            shape,
            identity,
            vec![0; shape.byte_len()],
            context.memory().clone(),
        )
        .map_err(|cause| ImageError::Source(Box::new(cause)))?;
        while !validator
            .validate_next(NonZeroUsize::MIN.saturating_add(4095))
            .map_err(|cause| ImageError::Source(Box::new(cause)))?
        {}
        validator.finish().map_err(|cause| ImageError::Source(Box::new(cause)))
    })
    .map_err(|cause| cause as Box<dyn Error>)
}
fn stamp(generation: u64) -> Result<FrameStamp, Box<dyn Error>> {
    Ok(FrameStamp { surface: SurfaceId::new(1)?, generation: Generation::new(generation)? })
}
fn pixels(extent: Extent, dx: i32, dy: i32) -> Vec<u32> {
    (0..extent.height())
        .flat_map(|y| {
            (0..extent.width()).map(move |x| {
                let x = (i64::from(x) - i64::from(dx)).rem_euclid(256) as u32;
                let y = (i64::from(y) - i64::from(dy)).rem_euclid(256) as u32;
                0xff000000 | (x << 16) | (y << 8) | ((x + 3 * y) & 255)
            })
        })
        .collect()
}
fn matches(dib: &mut Dib, expected: &[u32]) -> Result<(), Box<dyn Error>> {
    // SAFETY: This flushes only the current test thread's synchronous memory-DC operations,
    // before borrowing its original DIB pixels through the native storage owner.
    unsafe { GdiFlush().ok()? };
    for (actual, expected) in dib.pixels_mut()?.chunks_exact(4).zip(expected) {
        assert_eq!(
            u32::from_ne_bytes([actual[0], actual[1], actual[2], actual[3]]) & 0x00ffffff,
            expected & 0x00ffffff
        );
    }
    Ok(())
}
#[test]
fn actual_gdi_pan_repairs_and_native_exposure_match_complete_device_pixels()
-> Result<(), Box<dyn Error>> {
    let owner = admitted()?;
    let extent = Extent::new(31, 17)?;
    let mut retained = None;
    let dib = Dib::for_frame(&mut retained, &owner)?;
    for (dx, dy) in [(7, -3), (-6, 5), (-1, -1), (40, 0), (0, 0)] {
        let original = pixels(extent, 0, 0);
        let replacement = pixels(extent, dx, dy);
        let mut painter = Painter::default();
        let full = DamagePlan::full(extent);
        painter.paint(
            dib.dc(),
            &CpuFrame {
                kind: CpuFrameKind::Prepared,
                pending: false,
                target: stamp(1)?,
                base: None,
                damage: &full,
                pixels: &original,
            },
            false,
        )?;
        matches(dib, &original)?;
        let repair = DamagePlan::pan(extent, dx, dy)?;
        let frame = CpuFrame {
            kind: CpuFrameKind::Prepared,
            pending: false,
            target: stamp(2)?,
            base: Some(stamp(1)?),
            damage: &repair,
            pixels: &replacement,
        };
        painter.paint(dib.dc(), &frame, false)?;
        matches(dib, &replacement)?;
        dib.pixels_mut()?.fill(0);
        painter.paint(dib.dc(), &frame, true)?;
        matches(dib, &replacement)?;
    }
    dib.close()?;
    Ok(())
}
#[test]
fn clipped_native_presentation_cannot_become_a_future_blit_dependency() -> Result<(), Box<dyn Error>>
{
    let owner = admitted()?;
    let extent = Extent::new(31, 17)?;
    let mut retained = None;
    let dib = Dib::for_frame(&mut retained, &owner)?;
    let mut painter = Painter::default();
    let full = DamagePlan::full(extent);
    let original = pixels(extent, 0, 0);
    painter.paint(
        dib.dc(),
        &CpuFrame {
            kind: CpuFrameKind::Prepared,
            pending: false,
            target: stamp(1)?,
            base: None,
            damage: &full,
            pixels: &original,
        },
        false,
    )?;
    // SAFETY: Both clipping calls borrow this test's original live memory DC on its owner thread.
    let clipped = unsafe { IntersectClipRect(dib.dc(), 5, 3, 24, 14) };
    assert_ne!(clipped.0, 0);
    let replacement = pixels(extent, 7, -3);
    let repair = DamagePlan::pan(extent, 7, -3)?;
    painter.paint(
        dib.dc(),
        &CpuFrame {
            kind: CpuFrameKind::Prepared,
            pending: false,
            target: stamp(2)?,
            base: Some(stamp(1)?),
            damage: &repair,
            pixels: &replacement,
        },
        false,
    )?;
    assert_eq!(painter.last, None);
    let visible: Vec<_> = original
        .iter()
        .zip(&replacement)
        .enumerate()
        .map(|(index, (old, new))| {
            let x = index % extent.width() as usize;
            let y = index / extent.width() as usize;
            if (5..24).contains(&x) && (3..14).contains(&y) { *new } else { *old }
        })
        .collect();
    matches(dib, &visible)?;
    // SAFETY: Remove only the clipping region installed on this same owned memory DC above.
    let restored = unsafe { SelectClipRgn(dib.dc(), None) };
    assert_ne!(restored.0, 0);
    let final_pixels = pixels(extent, 9, -1);
    let repair = DamagePlan::pan(extent, 2, 2)?;
    painter.paint(
        dib.dc(),
        &CpuFrame {
            kind: CpuFrameKind::Prepared,
            pending: false,
            target: stamp(3)?,
            base: Some(stamp(2)?),
            damage: &repair,
            pixels: &final_pixels,
        },
        false,
    )?;
    matches(dib, &final_pixels)?;
    dib.close()?;
    Ok(())
}

#[test]
fn invalid_native_source_rejects_before_blitting_or_changing_the_retained_dependency()
-> Result<(), Box<dyn Error>> {
    let owner = admitted()?;
    let extent = Extent::new(31, 17)?;
    let original = pixels(extent, 0, 0);
    let mut retained = None;
    let dib = Dib::for_frame(&mut retained, &owner)?;
    let mut painter = Painter::default();
    let full = DamagePlan::full(extent);
    painter.paint(
        dib.dc(),
        &CpuFrame {
            kind: CpuFrameKind::Prepared,
            pending: false,
            target: stamp(1)?,
            base: None,
            damage: &full,
            pixels: &original,
        },
        false,
    )?;
    let repair = DamagePlan::pan(extent, 3, -2)?;
    let invalid = CpuFrame {
        kind: CpuFrameKind::Prepared,
        pending: false,
        target: stamp(2)?,
        base: Some(stamp(1)?),
        damage: &repair,
        pixels: &original[..original.len() - 1],
    };
    assert!(painter.paint(dib.dc(), &invalid, false).is_err());
    assert_eq!(painter.last, Some(stamp(1)?));
    matches(dib, &original)?;
    dib.close()?;
    Ok(())
}

#[test]
fn projected_native_pixels_cannot_become_an_exact_repair_dependency() -> Result<(), Box<dyn Error>>
{
    let owner = admitted()?;
    let extent = Extent::new(31, 17)?;
    let mut retained = None;
    let dib = Dib::for_frame(&mut retained, &owner)?;
    let mut painter = Painter::default();
    let full = DamagePlan::full(extent);
    let preview = vec![0xffd15ea5; extent.pixels() as usize];
    painter.paint(
        dib.dc(),
        &CpuFrame {
            kind: CpuFrameKind::Projected,
            pending: false,
            target: stamp(2)?,
            base: None,
            damage: &full,
            pixels: &preview,
        },
        false,
    )?;
    assert_eq!(painter.last, None);
    matches(dib, &preview)?;

    let prepared = pixels(extent, 7, -3);
    let forged_repair = DamagePlan::pan(extent, 7, -3)?;
    painter.paint(
        dib.dc(),
        &CpuFrame {
            kind: CpuFrameKind::Prepared,
            pending: false,
            target: stamp(3)?,
            base: Some(stamp(2)?),
            damage: &forged_repair,
            pixels: &prepared,
        },
        false,
    )?;
    matches(dib, &prepared)?;
    dib.close()?;
    Ok(())
}

#[test]
fn projected_frame_validation_rejects_base_blit_and_partial_damage() -> Result<(), Box<dyn Error>> {
    let extent = Extent::new(31, 17)?;
    let pixels = pixels(extent, 0, 0);
    let full = DamagePlan::full(extent);
    assert_eq!(
        CpuFrame {
            kind: CpuFrameKind::Projected,
            pending: false,
            target: stamp(2)?,
            base: None,
            damage: &full,
            pixels: &pixels,
        }
        .validate()?,
        extent
    );
    assert!(
        CpuFrame {
            kind: CpuFrameKind::Projected,
            pending: false,
            target: stamp(2)?,
            base: Some(stamp(1)?),
            damage: &full,
            pixels: &pixels,
        }
        .validate()
        .is_err()
    );

    let repair = DamagePlan::pan(extent, 3, -2)?;
    assert!(
        CpuFrame {
            kind: CpuFrameKind::Projected,
            pending: false,
            target: stamp(2)?,
            base: None,
            damage: &repair,
            pixels: &pixels,
        }
        .validate()
        .is_err()
    );

    let partial = DamagePlan::unchanged(extent);
    assert!(
        CpuFrame {
            kind: CpuFrameKind::Projected,
            pending: false,
            target: stamp(2)?,
            base: None,
            damage: &partial,
            pixels: &pixels,
        }
        .validate()
        .is_err()
    );
    Ok(())
}
