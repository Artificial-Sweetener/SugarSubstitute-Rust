// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Render actual installer composition into the immutable pixels its native peers will consume.
use super::*;
use sugarsubstitute_contracts::{
    execution::{Generation, MemoryRelease, SurfaceId},
    presentation::{FrameIdentity, FrameShape},
};
use sugarsubstitute_ui::scene::FramePaint;

struct Release;
impl MemoryRelease for Release {
    fn release(&self, _: u64) {}
}

#[test]
fn native_frame_keeps_pixels_and_matching_peers_through_publication() -> Result {
    let flow = SetupFlow::new("E:\\Substitute".into())?;
    let mut resources = SceneResources::new("en")?;
    for scale in [1.0, 1.25] {
        let view = view(1180, 760, scale);
        let shape = FrameShape::new(view.surface.width, view.surface.height, scale.into())?;
        let lease = MemoryLease::new(
            METADATA_BYTES + FramePaint::reservation_bytes(shape),
            Box::new(Release),
        );
        let scene = compose_flow(&flow, view, &mut resources, Some(&lease))?;
        let identity = FrameIdentity {
            surface: SurfaceId::new(1)?,
            state_generation: Generation::new(2)?,
            sequence: Generation::new(3)?,
        };
        let mut paint = scene.into_paint(identity, lease.clone())?;
        while !paint.step(&mut resources)? {}
        let Prepared { frame, layout } = paint.finish()?;
        assert_eq!(frame.shape(), shape);
        assert_eq!(frame.identity(), identity);
        let primary = layout
            .shell_controls
            .iter()
            .find(|peer| peer.id == Action::Primary.id())
            .ok_or("Primary peer absent")?;
        let x = ((primary.bounds.x + 5.0) * scale).round() as usize;
        let y = ((primary.bounds.y + 16.0) * scale).round() as usize;
        assert_eq!(
            &frame.pixels()[(y * shape.width() as usize + x) * 4..][..4],
            &[255, 64, 137, 255]
        );
        assert!(layout.page_controls.iter().any(|peer| peer.id == Action::Locale.id()));
        assert_eq!(&frame.pixels()[..4], &[0; 4]);
        drop(frame);
        assert!(lease.claim(lease.bytes()).is_err());
        drop(layout);
        assert!(lease.claim(lease.bytes()).is_ok());
    }
    Ok(())
}
