// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prove clipping, fractional phase, storage custody and publication of actual shared scenes.
use super::*;
use std::{
    error::Error,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use sugarsubstitute_contracts::{
    configuration::Theme,
    execution::{Generation, MemoryRelease, SurfaceId},
};
type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;

struct Release(Arc<AtomicUsize>);
impl MemoryRelease for Release {
    fn release(&self, _: u64) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
fn identity() -> Result<FrameIdentity> {
    Ok(FrameIdentity {
        surface: SurfaceId::new(1)?,
        state_generation: Generation::new(2)?,
        sequence: Generation::new(3)?,
    })
}
fn surface() -> PaintSpec {
    PaintSpec { theme: Theme::Dark, scale: 1.25, width: 120, height: 80 }
}
fn layer() -> Layer {
    Layer {
        viewport: Bounds { x: 8.2, y: 7.4, width: 50.0, height: 40.0 },
        elements: vec![Element::Decoration {
            bounds: Bounds { x: -5.0, y: -5.0, width: 65.0, height: 55.0 },
            appearance: Decoration::Button {
                primary: true,
                state: crate::widgets::ControlState::Rest,
            },
        }],
    }
}

#[test]
fn clipped_layer_preserves_fractional_coverage_and_premultiplied_pixels() -> Result {
    let s = surface();
    let shape = FrameShape::new(s.width, s.height, s.scale.into())?;
    let released = Arc::new(AtomicUsize::new(0));
    let lease =
        MemoryLease::new(FramePaint::reservation_bytes(shape), Box::new(Release(released.clone())));
    let mut paint = FramePaint::new(s, identity()?, vec![layer()], lease.clone())?;
    assert!(lease.claim(lease.bytes()).is_err());
    let mut fonts = SceneResources::new("en")?;
    let mut steps = 0;
    while !paint.step(&mut fonts)? {
        steps += 1;
    }
    assert!(steps > 4);
    let frame = paint.finish()?;
    assert_eq!(frame.identity(), identity()?);
    for y in 0..s.height as usize {
        for x in 0..s.width as usize {
            let pixel = &frame.pixels()[(y * s.width as usize + x) * 4..][..4];
            if !(10..73).contains(&x) || !(9..60).contains(&y) {
                assert_eq!(pixel, &[0; 4]);
            }
            assert!(pixel[..3].iter().all(|v| *v <= pixel[3]));
        }
    }
    let pixel = |x: usize, y: usize| &frame.pixels()[(y * 120 + x) * 4..][..4];
    assert_eq!(pixel(20, 20), &[255, 64, 137, 255]);
    assert_eq!(pixel(10, 20)[3], 191);
    assert_eq!(pixel(20, 9)[3], 191);
    assert_eq!(pixel(20, 59)[3], 64);
    drop(lease);
    assert_eq!(released.load(Ordering::SeqCst), 0);
    drop(frame);
    assert_eq!(released.load(Ordering::SeqCst), 1);
    Ok(())
}

#[test]
fn cancelled_or_rejected_surface_returns_all_admitted_storage() -> Result {
    let s = surface();
    let shape = FrameShape::new(s.width, s.height, s.scale.into())?;
    let lease = MemoryLease::new(
        FramePaint::reservation_bytes(shape),
        Box::new(Release(Arc::new(AtomicUsize::new(0)))),
    );
    let mut fonts = SceneResources::new("en")?;
    for steps in [0, 1, 4, 12, 28] {
        let mut paint = FramePaint::new(s, identity()?, vec![layer()], lease.clone())?;
        for _ in 0..steps {
            if paint.step(&mut fonts)? {
                break;
            }
        }
        drop(paint);
        assert!(lease.claim(lease.bytes()).is_ok());
    }
    assert!(FramePaint::new(s, identity()?, vec![layer()], lease.clone())?.finish().is_err());
    assert!(lease.claim(lease.bytes()).is_ok());
    let mut bad = layer();
    bad.viewport.x = f32::NAN;
    assert!(FramePaint::new(s, identity()?, vec![layer(), bad], lease.clone()).is_err());
    assert!(lease.claim(lease.bytes()).is_ok());
    let tiny = MemoryLease::new(
        METADATA_BYTES + shape.byte_len() as u64,
        Box::new(Release(Arc::new(AtomicUsize::new(0)))),
    );
    assert!(FramePaint::new(s, identity()?, vec![layer()], tiny.clone()).is_err());
    assert!(tiny.claim(tiny.bytes()).is_ok());
    Ok(())
}

#[test]
fn consecutive_layers_match_shared_source_over_and_clear_previous_pixels() -> Result {
    let s = surface();
    let shape = FrameShape::new(s.width, s.height, s.scale.into())?;
    let lease = MemoryLease::new(
        FramePaint::reservation_bytes(shape),
        Box::new(Release(Arc::new(AtomicUsize::new(0)))),
    );
    let viewport = Bounds { x: 0.0, y: 0.0, width: 96.0, height: 64.0 };
    let mut layers = vec![
        Layer {
            viewport,
            elements: vec![Element::Decoration { bounds: viewport, appearance: Decoration::Body }],
        },
        layer(),
    ];
    layers.push(Layer {
        viewport: Bounds { x: 80.0, y: 55.0, width: 10.0, height: 5.0 },
        elements: vec![],
    });
    let mut paint = FramePaint::new(s, identity()?, layers, lease)?;
    let mut fonts = SceneResources::new("en")?;
    while !paint.step(&mut fonts)? {}
    let frame = paint.finish()?;
    let mut expected = Bitmap::new(s.width, s.height).ok_or("Target absent")?;
    let mut base = SceneCursor::new(
        s,
        vec![Element::Decoration { bounds: viewport, appearance: Decoration::Body }],
        None,
    )?;
    while !base.step(&mut expected, &mut fonts)? {}
    let at = |x: usize, y: usize| (y * 120 + x) * 4;
    assert_eq!(&frame.pixels()[at(105, 72)..][..4], &expected.data()[at(105, 72)..][..4]);
    assert_eq!(&frame.pixels()[at(20, 20)..][..4], &[255, 64, 137, 255]);
    Ok(())
}
