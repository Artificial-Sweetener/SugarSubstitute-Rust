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
            &[255, 68, 132, 255]
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

/// The selected mirror checkbox uses the shared dark on-accent foreground, not artwork white.
#[test]
fn checked_mirror_glyph_uses_the_on_accent_foreground() -> Result {
    use sugarsubstitute_ui::icons::FluentIcon;
    let flow = SetupFlow::new("E:\\Substitute".into())?;
    let mut resources = SceneResources::new("en")?;
    for scale in [1.0, 1.25] {
        let view = view(1180, 760, scale);
        let shape = FrameShape::new(view.surface.width, view.surface.height, scale.into())?;
        let lease = MemoryLease::new(
            SNAPSHOT_BYTES + METADATA_BYTES + FramePaint::reservation_bytes(shape),
            Box::new(Release),
        );
        let mut state = Snapshot::capture(&flow, view, &lease)?;
        state.page = Page::Configuration;
        state.mode = RuntimeMode::Managed;
        state.mirrors = true;
        let scene = compose(&state, &mut resources, Some(&lease))?;
        let glyph = scene
            .page
            .iter()
            .find_map(|element| match element {
                Element::Icon { bounds, icon: FluentIcon::Accept, .. } => Some(*bounds),
                _ => None,
            })
            .ok_or("Checked mirror glyph absent")?;
        let left = ((scene.layout.viewport.x + glyph.x) * scale).ceil() as usize;
        let top = ((scene.layout.viewport.y + glyph.y) * scale).ceil() as usize;
        let right = ((scene.layout.viewport.x + glyph.x + glyph.width) * scale).floor() as usize;
        let bottom = ((scene.layout.viewport.y + glyph.y + glyph.height) * scale).floor() as usize;
        let fill_x = ((scene.layout.viewport.x + glyph.x - 2.0) * scale).round() as usize;
        let fill_y =
            ((scene.layout.viewport.y + glyph.y + glyph.height * 0.5) * scale).round() as usize;
        let identity = FrameIdentity {
            surface: SurfaceId::new(1)?,
            state_generation: Generation::new(2)?,
            sequence: Generation::new(3)?,
        };
        let mut paint = scene.into_paint(identity, lease)?;
        while !paint.step(&mut resources)? {}
        let Prepared { frame, .. } = paint.finish()?;
        let pixels = frame.pixels();
        let stride = shape.width() as usize;
        let sample = (top..bottom)
            .flat_map(|y| (left..right).map(move |x| &pixels[(y * stride + x) * 4..][..4]))
            .min_by_key(|pixel| pixel[0])
            .ok_or("Checked mirror glyph has no pixel region")?;
        let fill = &pixels[(fill_y * stride + fill_x) * 4..][..4];
        assert_eq!(fill, view.accent.variants(true).primary);
        assert!(
            fill[0].saturating_sub(sample[0]) >= 32,
            "Checked mirror glyph does not meaningfully darken its fill at {scale}"
        );
        assert_eq!(sample[3], 255);
        // The subpixel SVG stroke blends black coverage into the opaque checkbox fill.
        for (actual, channel) in sample[..3].iter().zip(&fill[..3]) {
            let expected = ((u16::from(*channel) * u16::from(sample[0]) + u16::from(fill[0]) / 2)
                / u16::from(fill[0])) as u8;
            assert!(actual.abs_diff(expected) <= 1, "Nonblack glyph blend at scale {scale}");
        }
    }
    Ok(())
}

/// Actual installer scenes preserve mode-specific primary text and rendered fill in every state.
#[test]
fn primary_button_uses_resolved_theme_for_foreground_and_pixels() -> Result {
    use sugarsubstitute_ui::widgets::ControlState;
    let flow = SetupFlow::new("D:\\SugarSubstitute".into())?;
    let mut resources = SceneResources::new("en")?;
    for theme in [Theme::Dark, Theme::Light] {
        for state in [ControlState::Rest, ControlState::Hover, ControlState::Pressed] {
            let mut view = view(1180, 760, 1.0);
            view.surface.theme = theme;
            if state != ControlState::Rest {
                view.interaction.hovered = Some(Action::Primary.id());
            }
            if state == ControlState::Pressed {
                view.interaction.pressed = Some(Action::Primary.id());
            }
            let shape = FrameShape::new(1180, 760, 1.0)?;
            let lease = MemoryLease::new(
                METADATA_BYTES + FramePaint::reservation_bytes(shape),
                Box::new(Release),
            );
            let scene = compose_flow(&flow, view, &mut resources, Some(&lease))?;
            let primary = scene
                .layout
                .shell_controls
                .iter()
                .find(|peer| peer.id == Action::Primary.id())
                .ok_or("Primary peer absent")?;
            let bounds = primary.bounds;
            let color = scene
                .shell
                .iter()
                .find_map(|element| match element {
                    Element::Label { layout, .. }
                        if layout.origin == [bounds.x + 12.0, bounds.y + 6.0] =>
                    {
                        Some(layout.color)
                    }
                    _ => None,
                })
                .ok_or("Primary label absent")?;
            let channel = if theme == Theme::Dark { 0 } else { 255 };
            assert_eq!(
                color,
                [channel, channel, channel, if state == ControlState::Pressed { 161 } else { 255 }]
            );
            let identity = FrameIdentity {
                surface: SurfaceId::new(1)?,
                state_generation: Generation::new(2)?,
                sequence: Generation::new(3)?,
            };
            let mut paint = scene.into_paint(identity, lease)?;
            while !paint.step(&mut resources)? {}
            let Prepared { frame, .. } = paint.finish()?;
            let x = (bounds.x + 5.0) as usize;
            let y = (bounds.y + bounds.height * 0.5) as usize;
            assert_eq!(
                &frame.pixels()[((y * 1180 + x) * 4)..][..4],
                &view.accent.variants(theme == Theme::Dark).button(state)
            );
        }
    }
    Ok(())
}
