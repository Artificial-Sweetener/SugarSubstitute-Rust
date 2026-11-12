// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Shared native caption-button backgrounds, independent of surface placement.

use crate::bitmap::Bitmap;
use crate::shapes::{AllocationError, blend_pixel};

/// Native caption action determines destructive hover feedback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptionAction {
    /// Standard close action.
    Close,
    /// A toggle or another non-destructive caption action.
    Auxiliary,
}

/// Observed pointer and toggle state; interaction ownership remains with the host.
#[derive(Clone, Copy, Debug, Default)]
pub struct CaptionState {
    /// Pointer lies inside this button.
    pub hovered: bool,
    /// The owning input transaction is pressed.
    pub pressed: bool,
    /// The associated panel is open.
    pub selected: bool,
}

/// One standard 46-by-32-logical-pixel caption button.
#[derive(Clone, Copy, Debug)]
pub struct CaptionButton {
    /// Logical left edge within the destination.
    pub x: f32,
    /// Logical top edge within the destination.
    pub y: f32,
    /// Select native close or auxiliary feedback.
    pub action: CaptionAction,
    /// Use the dark-window palette.
    pub dark: bool,
    /// Current input state.
    pub state: CaptionState,
}

impl CaptionButton {
    /// Paint one bounded native caption background on a preparation worker.
    /// Child-widget origins snap independently from their fractional far edges.
    /// At scale four this touches at most 23,552 destination pixels.
    /// # Errors
    /// Rejects invalid geometry or scale before changing the destination.
    pub fn paint(self, target: &mut Bitmap, scale: f32) -> Result<(), AllocationError> {
        if ![self.x, self.y, scale].iter().all(|value| value.is_finite())
            || self.x < 0.0
            || self.y < 0.0
            || !(0.5..=4.0).contains(&scale)
            || (self.x + 46.0) * scale > 4096.0
            || (self.y + 32.0) * scale > 4096.0
        {
            return Err(AllocationError::InvalidDimensions);
        }
        let color = self.color();
        let left = (self.x * scale).round();
        let top = (self.y * scale).round();
        let right = (self.x + 46.0) * scale;
        let bottom = (self.y + 32.0) * scale;
        for row in top as u32..bottom.ceil().min(target.height() as f32) as u32 {
            for column in left as u32..right.ceil().min(target.width() as f32) as u32 {
                let horizontal = (right - column as f32).clamp(0.0, 1.0);
                let vertical = (bottom - row as f32).clamp(0.0, 1.0);
                let coverage = (horizontal * vertical * 255.0).floor() as u8;
                blend_pixel(target, i64::from(column), i64::from(row), color, coverage);
            }
        }
        Ok(())
    }

    /// Resolve state precedence once for all native caption consumers.
    fn color(self) -> [u8; 4] {
        if self.action == CaptionAction::Close && (self.state.pressed || self.state.hovered) {
            return if self.state.pressed { [241, 112, 122, 255] } else { [232, 17, 35, 255] };
        }
        let foreground = if self.dark { 255 } else { 0 };
        let alpha = if self.state.pressed {
            51
        } else if self.state.hovered {
            26
        } else if self.action == CaptionAction::Auxiliary && self.state.selected {
            if self.dark { 28 } else { 18 }
        } else {
            0
        };
        [foreground, foreground, foreground, alpha]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_geometry_leaves_destination_unchanged() -> Result<(), Box<dyn std::error::Error>> {
        let mut pixels = Bitmap::new(80, 80).ok_or("Caption target absent")?;
        pixels.fill(tiny_skia::Color::WHITE);
        for (x, y, scale) in [(f32::NAN, 0.0, 1.0), (0.0, -1.0, 1.0), (0.0, 0.0, 5.0)] {
            let button = CaptionButton {
                x,
                y,
                action: CaptionAction::Close,
                dark: true,
                state: CaptionState { hovered: true, ..CaptionState::default() },
            };
            assert!(button.paint(&mut pixels, scale).is_err());
            assert!(pixels.data().iter().all(|byte| *byte == 255));
        }
        Ok(())
    }

    #[test]
    fn caption_can_move_without_leaving_its_child_rectangle()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut pixels = Bitmap::new(100, 100).ok_or("Caption target absent")?;
        CaptionButton {
            x: 8.0,
            y: 12.0,
            action: CaptionAction::Auxiliary,
            dark: true,
            state: CaptionState { selected: true, ..CaptionState::default() },
        }
        .paint(&mut pixels, 1.25)?;
        for y in 0..100 {
            for x in 0..100 {
                let alpha = pixels.pixel(x, y).ok_or("Caption pixel absent")?.alpha();
                if !(10..68).contains(&x) || !(15..55).contains(&y) {
                    assert_eq!(alpha, 0);
                } else if x < 67 {
                    assert_eq!(alpha, 28);
                }
            }
        }
        Ok(())
    }
}
