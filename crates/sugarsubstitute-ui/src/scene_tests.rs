// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify shared scene ordering, rejection and retained preparation memory.
use super::*;
use crate::{
    progress::ProgressAppearance,
    text::{TextAlignment, TextMetrics, VerticalAnchor},
    widgets::ControlState,
};
use std::{
    error::Error,
    sync::Arc,
    sync::atomic::{AtomicUsize, Ordering},
};
use sugarsubstitute_contracts::{configuration::Theme, execution::MemoryRelease};

fn surface() -> PaintSpec {
    PaintSpec { theme: Theme::Dark, scale: 1.0, width: 160, height: 80 }
}

struct Release(Arc<AtomicUsize>);
impl MemoryRelease for Release {
    fn release(&self, _: u64) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn cancelled_scene_releases_its_masks_without_publishing_a_frame() -> Result<(), Box<dyn Error>> {
    let released = Arc::new(AtomicUsize::new(0));
    let lease = MemoryLease::new(160 * 80 * 2, Box::new(Release(released.clone())));
    let cursor = SceneCursor::new(surface(), AccentPalette::PRODUCT, vec![], Some(lease.clone()))?;
    assert!(lease.claim(1).is_err());
    drop(cursor);
    assert!(lease.claim(160 * 80 * 2).is_ok());
    drop(lease);
    assert_eq!(released.load(Ordering::SeqCst), 1);
    Ok(())
}

#[test]
fn scene_checks_late_commands_before_admitting_scratch() -> Result<(), Box<dyn Error>> {
    let lease = MemoryLease::new(160 * 80 * 2, Box::new(Release(Arc::new(AtomicUsize::new(0)))));
    let elements = vec![
        Element::Decoration {
            bounds: Bounds { x: 0.0, y: 0.0, width: 160.0, height: 80.0 },
            appearance: Decoration::Body,
        },
        Element::Icon {
            bounds: Bounds { x: 0.0, y: 0.0, width: 3000.0, height: 1.0 },
            icon: FluentIcon::Home,
        },
    ];
    assert!(
        SceneCursor::new(surface(), AccentPalette::PRODUCT, elements, Some(lease.clone())).is_err()
    );
    assert!(lease.claim(160 * 80 * 2).is_ok());
    Ok(())
}

#[test]
fn scene_composes_real_widgets_labels_and_icons_in_order() -> Result<(), Box<dyn Error>> {
    let b = Bounds { x: 8.0, y: 8.0, width: 120.0, height: 32.0 };
    let elements = vec![
        Element::Decoration {
            bounds: b,
            appearance: Decoration::Button { primary: true, state: ControlState::Rest },
        },
        Element::Label {
            text: "Continue".into(),
            layout: LabelLayout {
                origin: [b.x, b.y + 5.0],
                extent: [b.width, 24.0],
                clip: None,
                anchor: VerticalAnchor::Top,
                alignment: TextAlignment::Center,
                metrics: TextMetrics::Natural,
                font_size: 14.0,
                line_height: 19.0,
                weight: 400,
                color: [0, 0, 0, 255],
            },
        },
        Element::Icon {
            bounds: Bounds { x: 136.5, y: 16.5, width: 16.0, height: 16.0 },
            icon: FluentIcon::Accept,
        },
        Element::Progress {
            bar: ProgressBar {
                x: 8.0,
                y: 60.0,
                width: 120.0,
                height: 6.0,
                appearance: ProgressAppearance::Wizard,
            },
            frame: ProgressFrame { completed: 0.5, activity: None },
        },
    ];
    let mut cursor = SceneCursor::new(surface(), AccentPalette::PRODUCT, elements, None)?;
    let mut target = Bitmap::new(160, 80).ok_or("Scene absent")?;
    let mut fonts = SceneResources::new("en")?;
    while !cursor.step(&mut target, &mut fonts)? {}
    assert_eq!(target.pixel(12, 20).ok_or("Button absent")?.alpha(), 255);
    assert!((30..105).any(|x| {
        (15..33).any(|y| target.pixel(x, y).is_some_and(|p| p.red() < 64 && p.alpha() == 255))
    }));
    assert!(
        (136..154).any(|x| (16..34).any(|y| target.pixel(x, y).is_some_and(|p| p.alpha() > 0)))
    );
    assert_eq!(target.pixel(12, 62).ok_or("Progress absent")?.alpha(), 255);
    assert!(target.pixel(120, 62).ok_or("Track absent")?.alpha() < 255);
    assert_eq!(target.pixel(0, 0).ok_or("Outside absent")?.alpha(), 0);
    Ok(())
}

/// One scene accent reaches both its decorations and its progress elements.
#[test]
fn scene_accent_reaches_decorations_and_progress() -> Result<(), Box<dyn Error>> {
    let accent =
        AccentPalette::system(sugarsubstitute_contracts::appearance::AccentColor::new(0, 120, 212));
    let elements = vec![
        Element::Decoration {
            bounds: Bounds { x: 8.0, y: 8.0, width: 120.0, height: 32.0 },
            appearance: Decoration::Button { primary: true, state: ControlState::Rest },
        },
        Element::Progress {
            bar: ProgressBar {
                x: 8.0,
                y: 60.0,
                width: 120.0,
                height: 6.0,
                appearance: ProgressAppearance::Wizard,
            },
            frame: ProgressFrame { completed: 0.5, activity: None },
        },
    ];
    let mut cursor = SceneCursor::new(surface(), accent, elements, None)?;
    let mut target = Bitmap::new(160, 80).ok_or("Scene absent")?;
    let mut fonts = SceneResources::new("en")?;
    while !cursor.step(&mut target, &mut fonts)? {}
    for (x, y) in [(40, 20), (12, 62)] {
        let pixel = target.pixel(x, y).ok_or("Accent pixel absent")?;
        assert_eq!([pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()], [0, 120, 212, 255]);
    }
    Ok(())
}
