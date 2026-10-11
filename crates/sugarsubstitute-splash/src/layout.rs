// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Own source-compatible logical geometry independently of the native window.

use crate::{Bounds, Theme};

/// Preserve the PySide splash's fixed logical width and height.
pub const LOGICAL_SIZE: u32 = 558;

/// Identify a splash-owned control without depending on a native event type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    /// Console disclosure immediately beside close.
    ConsoleToggle,
    /// Startup cancellation affordance.
    Close,
}

/// Supply portable appearance state; process and connection state stay elsewhere.
#[derive(Clone, Copy, Debug)]
pub struct Appearance {
    /// Current presentation theme.
    pub theme: Theme,
    /// Expose the blank output surface and lift the character.
    pub expanded: bool,
    /// Control under the pointer.
    pub hovered: Option<Control>,
    /// Currently pressed control.
    pub pressed: Option<Control>,
}

impl Appearance {
    /// Open the requested blank-console proof with the source dark theme.
    pub const fn demo() -> Self {
        Self { theme: Theme::Dark, expanded: true, hovered: None, pressed: None }
    }

    /// Return the source widget's whole mascot slot before aspect fitting.
    pub const fn mascot(self) -> Bounds {
        if self.expanded {
            Bounds { x: 83.0, y: 7.0, width: 387.0, height: 386.0 }
        } else {
            Bounds { x: 85.0, y: 86.0, width: 387.0, height: 386.0 }
        }
    }

    /// Return the fixed console or collapsed feedback panel rectangle.
    pub const fn feedback(self) -> Bounds {
        Bounds {
            x: 24.0,
            y: if self.expanded { 356.0 } else { 486.0 },
            width: 510.0,
            height: if self.expanded { 178.0 } else { 48.0 },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Appearance;

    /// Keep disclosure's two reference positions independent of presentation timers.
    #[test]
    fn disclosure_reflows_only_the_reference_geometry() {
        let mut appearance = Appearance::demo();
        assert_eq!(appearance.mascot().y, 7.0);
        assert_eq!(appearance.feedback().height, 178.0);
        appearance.expanded = false;
        assert_eq!(appearance.mascot().y, 86.0);
        assert_eq!(appearance.feedback().y, 486.0);
    }
}
