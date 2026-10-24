// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reconcile asynchronous document placement with the current native viewport.
use sugarsubstitute_desktop::installer::InstallerConsoleLayout;

/// Clip an older document measurement after resize; minimized windows retain a hidden surface.
pub(super) fn clip(mut layout: InstallerConsoleLayout) -> super::Result<InstallerConsoleLayout> {
    let bounds = &mut layout.bounds;
    if !layout.scale.is_finite()
        || layout.scale <= 0.0
        || layout.scale > 4.0
        || layout.width > 4096
        || layout.height > 4096
        || [bounds.x, bounds.y, bounds.width, bounds.height]
            .iter()
            .any(|value| !value.is_finite() || *value < 0.0)
    {
        return Err("Invalid installer console placement".into());
    }
    layout.visible &= layout.width > 0 && layout.height > 0;
    layout.width = layout.width.max(1);
    layout.height = layout.height.max(1);
    let width = f64::from(layout.width) / layout.scale;
    let height = f64::from(layout.height) / layout.scale;
    bounds.x = bounds.x.min(width);
    bounds.y = bounds.y.min(height);
    bounds.width = bounds.width.min(width - bounds.x);
    bounds.height = bounds.height.min(height - bounds.y);
    layout.visible &= bounds.width > 0.0 && bounds.height > 0.0;
    Ok(layout)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sugarsubstitute_desktop::ControlBounds;

    fn placement() -> InstallerConsoleLayout {
        InstallerConsoleLayout {
            width: 720,
            height: 560,
            scale: 1.0,
            bounds: ControlBounds { x: 300.0, y: 400.0, width: 800.0, height: 300.0 },
            visible: true,
        }
    }

    #[test]
    fn queued_pre_resize_bounds_clip_and_fresh_bounds_restore() -> super::super::Result<()> {
        let previous = placement();
        let clipped = clip(previous)?;
        assert!(clipped.visible);
        assert_eq!(clipped.bounds.width, 420.0);
        assert_eq!(clipped.bounds.height, 160.0);
        let mut fresh = previous;
        fresh.width = 1180;
        fresh.height = 760;
        assert_eq!(clip(fresh)?, fresh);
        fresh.scale = 2.0;
        assert!(!clip(fresh)?.visible);
        Ok(())
    }

    #[test]
    fn minimize_hides_console_without_invalid_frame_dimensions() -> super::super::Result<()> {
        let mut minimized = placement();
        minimized.width = 0;
        minimized.height = 0;
        let hidden = clip(minimized)?;
        assert!(!hidden.visible);
        assert_eq!((hidden.width, hidden.height), (1, 1));
        assert!(clip(placement())?.visible);
        Ok(())
    }

    #[test]
    fn malformed_document_coordinates_remain_rejected() {
        for value in [f64::NAN, f64::INFINITY, -1.0] {
            let mut layout = placement();
            layout.bounds.width = value;
            assert!(clip(layout).is_err());
        }
    }
}
