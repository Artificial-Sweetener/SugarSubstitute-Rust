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
    /// Retry after actual previous-attempt settlement.
    Retry,
}

/// Closed presentation of the actual application recovery phase.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum StartupStatus {
    /// Normal startup is still in progress.
    #[default]
    Loading,
    /// Startup remains active after two minutes; elapsed time grants no failure authority.
    LoadingLong,
    /// Startup remains active after five minutes and may still be preparing dependencies.
    LoadingExtended,
    /// The application retained an original startup failure.
    Failed,
    /// Original attempt cleanup remains unresolved.
    Settling,
    /// Real cleanup settled and the application can accept retry.
    RetryReady,
    /// Cleanup failed and original ownership remains retained.
    CleanupFailed,
    /// A new version is being applied before the application opens.
    Updating(crate::UpdateVersion),
    /// The host is updating SugarCubes before ComfyUI loads it.
    UpdatingSugarCubes,
}

/// Supply portable appearance state; process and connection state stay elsewhere.
#[derive(Clone, Copy, Debug)]
pub struct Appearance {
    /// Current presentation theme.
    pub theme: Theme,
    /// Accent roles resolved from the operating-system accent, or the product fallback.
    pub accent: sugarsubstitute_ui::accent::AccentPalette,
    /// Localized application recovery presentation; this grants no cleanup authority.
    pub status: StartupStatus,
    /// Latest observed startup operation, localized independently of elapsed time.
    pub operation: sugarsubstitute_contracts::startup::StartupOperation,
    /// Requested disclosure state used by the caption affordance.
    pub expanded: bool,
    /// Sampled console expansion in the inclusive range zero to one.
    pub reveal: f64,
    /// Control under the pointer.
    pub hovered: Option<Control>,
    /// Currently pressed control.
    pub pressed: Option<Control>,
}

impl Appearance {
    /// Open the requested blank-console proof with the source dark theme and product accent.
    pub const fn demo() -> Self {
        Self {
            theme: Theme::Dark,
            accent: sugarsubstitute_ui::accent::AccentPalette::PRODUCT,
            status: StartupStatus::Loading,
            operation: sugarsubstitute_contracts::startup::StartupOperation::Preparing,
            expanded: true,
            reveal: 1.0,
            hovered: None,
            pressed: None,
        }
    }

    /// Return the source widget's whole mascot slot before aspect fitting.
    pub const fn mascot(self) -> Bounds {
        Bounds {
            x: 85.0 - 2.0 * self.reveal,
            y: 86.0 - 79.0 * self.reveal,
            width: 387.0,
            height: 386.0,
        }
    }

    /// Return the console panel growing upward from the fixed lower edge.
    pub const fn feedback(self) -> Bounds {
        Bounds { x: 24.0, y: self.bar_y(), width: 510.0, height: 534.0 - self.bar_y() }
    }

    /// Keep the single progress bar attached to the moving console edge.
    pub const fn bar_y(self) -> f64 {
        530.0 - 174.0 * self.reveal
    }

    /// Simplified feedback appears only after the full console finishes closing.
    pub fn status_visible(self) -> bool {
        !self.expanded && self.reveal == 0.0
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
        appearance.reveal = 0.0;
        assert_eq!(appearance.mascot().y, 86.0);
        assert_eq!(appearance.feedback().y, 530.0);
    }
}
